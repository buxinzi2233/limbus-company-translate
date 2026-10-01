//! Shared real-SQLite fixture setup for authorization integration tests.
use anyhow::Result;
use loc_core::{DraftGrantRequest, DraftPermit, ScopeManifest, Store};
use serde_json::json;

pub fn scope_all(store: &Store, project: &str) -> Result<()> {
    if store.latest_source_import_id(project)? == 0 {
        store.raw_connection().lock().unwrap().execute(
            "INSERT INTO source_imports(project_id,source_directory,translation_directory,active_segments,added_or_reactivated,changed,retired,created_at)
            VALUES(?1,'/test/kr','/test/zh',0,0,0,0,?2)", [project, &chrono::Utc::now().to_rfc3339()],
        )?;
    }
    store.raw_connection().lock().unwrap().execute(
        "UPDATE segments SET source_identity=key_path WHERE project_id=?1 AND source_identity IS NULL", [project],
    )?;
    let rows = store
        .query_segments_paged(project, None, None, None, 0, 100_000)?
        .1;
    let files = store.get_files(project)?;
    let identities: Vec<String> = {
        let conn = store.raw_connection();
        let conn = conn.lock().unwrap();
        rows.iter()
            .map(|r| {
                Ok(conn.query_row(
                    "SELECT source_identity FROM segments WHERE id=?1",
                    [r.id],
                    |row| row.get(0),
                )?)
            })
            .collect::<Result<_>>()?
    };
    let manifest: ScopeManifest = serde_json::from_value(json!({
        "project_id":project,
        "files":files.iter().map(|f|json!({"path":f.relative_path,"sha256":"a".repeat(64),"selected_segments":rows.iter().filter(|r|r.file_id==f.id).count()})).collect::<Vec<_>>(),
        "segments":rows.iter().zip(identities).map(|(r,identity)|json!({"segment_id":r.id,"path":r.relative_path,"pointer":r.key_path,"source_kr":r.source_kr,"source_identity":identity})).collect::<Vec<_>>()
    }))?;
    store.import_task_scope(
        &manifest,
        "Integration scope",
        &"a".repeat(64),
        store.latest_source_import_id(project)?,
    )?;
    Ok(())
}

pub fn grant(store: &Store, project: &str, ids: Vec<i64>) -> Result<DraftPermit> {
    if store.task_scope(project)?.is_none() {
        scope_all(store, project)?;
    }
    let scope = store.task_scope(project)?.unwrap();
    Ok(store
        .create_draft_grant(&DraftGrantRequest {
            project_id: project.into(),
            worker: "integration".into(),
            manifest_sha256: scope.manifest_sha256,
            source_import_id: scope.source_import_id,
            segment_ids: ids,
        })?
        .1)
}
