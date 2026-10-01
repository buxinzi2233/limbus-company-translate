use super::{
    segments::segment_from_row,
    store::{SegmentWithFile, Store},
};
use crate::{FileRecord, Project, Segment};
use anyhow::{ensure, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};

pub struct ImportFile {
    pub identities: BTreeMap<String, String>,
    pub file: FileRecord,
    pub segments: Vec<Segment>,
}

impl Store {
    /// Preserve existing project metadata; commit all prepared files and their FTS changes together.
    pub fn import_files(
        &self,
        project: &Project,
        files: &[ImportFile],
    ) -> Result<Vec<SegmentWithFile>> {
        let mut conn = self.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
            [&project.id],
            |row| row.get(0),
        )?;
        if !exists {
            upsert_project(&tx, project)?;
        }
        let before: BTreeMap<i64, String> = {
            let mut stmt = tx.prepare(
                "SELECT id, source_kr FROM segments WHERE project_id = ?1 AND active = 1",
            )?;
            let rows = stmt.query_map([&project.id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        // Park all paths before reassignment so arbitrary array permutations cannot collide.
        tx.execute(
            "UPDATE segments SET retired_key_path = key_path, key_path = '@retired:' || id,
            active = 0 WHERE project_id = ?1 AND active = 1",
            [&project.id],
        )?;
        tx.execute(
            "UPDATE files SET active = 0 WHERE project_id = ?1",
            [&project.id],
        )?;
        let mut imported = Vec::new();
        let mut paths = BTreeSet::new();
        for prepared in files {
            ensure!(
                prepared.file.project_id == project.id,
                "Import file belongs to a different project: {}",
                prepared.file.relative_path
            );
            ensure!(
                paths.insert(&prepared.file.relative_path),
                "Duplicate file in import: {}",
                prepared.file.relative_path
            );
            let file_id = upsert_file(&tx, &prepared.file)?;
            let segments: Vec<Segment> = prepared
                .segments
                .iter()
                .map(|segment| Segment {
                    file_id,
                    ..segment.clone()
                })
                .collect();
            let pointers: BTreeSet<_> = segments
                .iter()
                .map(|segment| segment.key_path.as_str())
                .collect();
            ensure!(
                pointers.len() == segments.len(),
                "Duplicate segment pointer in import: {}",
                prepared.file.relative_path
            );
            ensure!(
                prepared.identities.len() == segments.len(),
                "Import identity count differs from segment count: {}",
                prepared.file.relative_path
            );
            let unique: BTreeSet<_> = prepared.identities.values().collect();
            ensure!(
                unique.len() == segments.len(),
                "Duplicate source identity: {}",
                prepared.file.relative_path
            );
            let locked: bool = tx.query_row(
                "SELECT is_locked FROM files WHERE id = ?1",
                [file_id],
                |row| row.get(0),
            )?;
            for segment in &segments {
                let identity = prepared
                    .identities
                    .get(&segment.key_path)
                    .ok_or_else(|| anyhow::anyhow!("Missing identity for {}", segment.key_path))?;
                let previous = tx
                    .query_row(
                        "SELECT id, source_kr, stage, source_identity IS NULL FROM segments
                    WHERE file_id = ?1 AND (source_identity = ?2 OR
                    (source_identity IS NULL AND retired_key_path = ?3))",
                        params![file_id, identity, segment.key_path],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i32>(2)?,
                                row.get::<_, bool>(3)?,
                            ))
                        },
                    )
                    .optional()?;
                let requires_array_review =
                    identity.contains("\"Entry\"") || identity.contains("\"SnapshotEntry\"");
                if let Some((id, source, stage, legacy)) = previous {
                    ensure!(
                        !(locked || stage == 9) || source == segment.source_kr,
                        "Source changed in locked segment {id}; unlock and review before importing"
                    );
                    ensure!(!(locked || stage == 9) || !legacy || !requires_array_review,
                        "Locked legacy array segment {id} has no verified identity; unlock before importing");
                    tx.execute(
                        "UPDATE segments SET key_path = ?1, active = 1, source_identity = ?2,
                        retired_key_path = NULL, updated_at = ?3 WHERE id = ?4",
                        params![
                            segment.key_path,
                            identity,
                            segment.updated_at.to_rfc3339(),
                            id
                        ],
                    )?;
                    if legacy && requires_array_review {
                        tx.execute("UPDATE segments SET stage = 2, review_confirmed = 0, qa_passed = 0,
                            qa_issues = 'Legacy array identity requires review' WHERE id = ?1 AND target_zh IS NOT NULL", [id])?;
                    }
                } else {
                    ensure!(
                        !locked,
                        "Cannot add a new segment to locked file {}",
                        prepared.file.relative_path
                    );
                }
            }
            upsert_segments(&tx, &segments)?;
            for segment in &segments {
                tx.execute("UPDATE segments SET source_identity = ?1, active = 1 WHERE file_id = ?2 AND key_path = ?3",
                    params![prepared.identities[&segment.key_path], file_id, segment.key_path])?;
            }
            let mut query = tx.prepare("SELECT s.id, s.project_id, s.file_id, f.relative_path,
                s.key_path, s.source_kr, s.previous_kr, s.target_zh, s.speaker, s.context_info,
                s.stage, s.qa_passed, s.qa_issues, s.updated_at, f.is_locked, s.review_confirmed
                FROM segments s JOIN files f ON f.id = s.file_id WHERE s.active = 1 AND s.project_id = ?1 AND s.file_id = ?2 ORDER BY s.id")?;
            let rows = query.query_map(params![project.id, file_id], segment_from_row)?;
            for row in rows {
                let segment = row?;
                if pointers.contains(segment.key_path.as_str()) {
                    imported.push(segment);
                }
            }
        }
        let frozen_retired: Vec<i64> = tx
            .prepare("SELECT id FROM segments WHERE project_id = ?1 AND active = 0 AND stage = 9")?
            .query_map([&project.id], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in frozen_retired {
            ensure!(!before.contains_key(&id),
                "Cannot retire frozen segment {id} after source identity changed; unlock and review before importing");
        }
        let current_ids: BTreeSet<_> = imported.iter().map(|item| item.id).collect();
        let added = imported
            .iter()
            .filter(|item| !before.contains_key(&item.id))
            .count();
        let changed = imported
            .iter()
            .filter(|item| {
                before
                    .get(&item.id)
                    .is_some_and(|source| source != &item.source_kr)
            })
            .count();
        let retired = before.keys().filter(|id| !current_ids.contains(id)).count();
        if let (Some(source), Some(translation)) =
            (&project.live_client_kr_dir, &project.workspace_zh_dir)
        {
            tx.execute(
                "UPDATE projects SET live_client_kr_dir = ?1, workspace_zh_dir = ?2 WHERE id = ?3",
                params![source, translation, project.id],
            )?;
            tx.execute("INSERT INTO source_imports (project_id, source_directory, translation_directory,
                active_segments, added_or_reactivated, changed, retired, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![project.id, source, translation, imported.len(), added, changed, retired, chrono::Utc::now().to_rfc3339()])?;
        }
        tx.commit()?;
        Ok(imported)
    }
}

pub(super) fn upsert_project(conn: &Connection, project: &Project) -> Result<()> {
    conn.execute(
            r#"
            INSERT INTO projects (id, name, live_client_kr_dir, upstream_llc_dir, workspace_zh_dir, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                live_client_kr_dir = excluded.live_client_kr_dir,
                upstream_llc_dir = excluded.upstream_llc_dir,
                workspace_zh_dir = excluded.workspace_zh_dir,
                updated_at = excluded.updated_at
            "#,
            params![
                project.id,
                project.name,
                project.live_client_kr_dir,
                project.upstream_llc_dir,
                project.workspace_zh_dir,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
            ],
        )?;
    Ok(())
}

pub(super) fn upsert_file(conn: &Connection, file: &FileRecord) -> Result<i64> {
    conn.execute(
            r#"
            INSERT INTO files (project_id, relative_path, category, total_segments, translated_segments, reviewed_segments, is_locked, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(project_id, relative_path) DO UPDATE SET
                active = 1,
                category = excluded.category,
                total_segments = excluded.total_segments,
                translated_segments = excluded.translated_segments,
                reviewed_segments = excluded.reviewed_segments,
                is_locked = MAX(files.is_locked, excluded.is_locked),
                updated_at = excluded.updated_at
            "#,
            params![
                file.project_id,
                file.relative_path,
                file.category,
                file.total_segments,
                file.translated_segments,
                file.reviewed_segments,
                if file.is_locked { 1 } else { 0 },
                file.created_at.to_rfc3339(),
                file.updated_at.to_rfc3339(),
            ],
        )?;

    let id = conn.query_row(
        "SELECT id FROM files WHERE project_id = ?1 AND relative_path = ?2",
        params![file.project_id, file.relative_path],
        |row| row.get(0),
    )?;
    Ok(id)
}

pub(super) fn upsert_segments(conn: &Connection, segments: &[Segment]) -> Result<usize> {
    let mut count = 0;
    {
        let mut stmt = conn.prepare(
                r#"
                INSERT INTO segments (
                    project_id, file_id, key_path, source_kr, previous_kr,
                    target_zh, speaker, context_info, stage, qa_passed,
                    qa_issues, created_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ON CONFLICT(file_id, key_path) DO UPDATE SET
                    review_confirmed = CASE WHEN segments.source_kr != excluded.source_kr THEN 0 ELSE segments.review_confirmed END,
                    source_kr = excluded.source_kr,
                    previous_kr = CASE WHEN segments.source_kr != excluded.source_kr THEN segments.source_kr ELSE segments.previous_kr END,
                    target_zh = COALESCE(segments.target_zh, excluded.target_zh),
                    speaker = excluded.speaker,
                    context_info = excluded.context_info,
                    stage = CASE WHEN segments.source_kr != excluded.source_kr THEN 2
                        WHEN segments.target_zh IS NOT NULL THEN segments.stage ELSE excluded.stage END,
                    qa_passed = CASE WHEN segments.source_kr != excluded.source_kr THEN 0
                        WHEN segments.target_zh IS NOT NULL THEN segments.qa_passed ELSE excluded.qa_passed END,
                    qa_issues = CASE WHEN segments.source_kr != excluded.source_kr THEN 'Source changed; review the existing translation'
                        WHEN segments.target_zh IS NOT NULL THEN segments.qa_issues ELSE excluded.qa_issues END,
                    updated_at = excluded.updated_at
                WHERE segments.stage != 9
                    AND NOT EXISTS (SELECT 1 FROM files WHERE id = segments.file_id AND is_locked = 1)
                "#,
            )?;

        for seg in segments {
            let owner: String = conn.query_row(
                "SELECT project_id FROM files WHERE id = ?1",
                [seg.file_id],
                |row| row.get(0),
            )?;
            anyhow::ensure!(
                owner == seg.project_id,
                "Segment project {} does not own file {}",
                seg.project_id,
                seg.file_id
            );
            stmt.execute(params![
                seg.project_id,
                seg.file_id,
                seg.key_path,
                seg.source_kr,
                seg.previous_kr,
                seg.target_zh,
                seg.speaker,
                seg.context_info,
                seg.stage as i32,
                if seg.qa_passed { 1 } else { 0 },
                seg.qa_issues,
                seg.created_at.to_rfc3339(),
                seg.updated_at.to_rfc3339(),
            ])?;
            count += 1;
        }
    }
    Ok(count)
}
