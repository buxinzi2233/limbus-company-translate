use super::{
    segments::segment_from_row,
    store::{SegmentWithFile, Store},
};
use anyhow::Result;
use rusqlite::params;

pub(super) const REVIEW_FILTER: &str = "s.active=1 AND f.is_locked=0 AND
    (s.stage IN (1,2,3,5) OR (s.stage=4 AND (s.review_confirmed=0 OR s.qa_passed=0)))";

impl Store {
    /// Return a consistent, bounded queue; locked rows never displace actionable work.
    pub fn review_queue(&self, project_id: &str) -> Result<(usize, Vec<SegmentWithFile>)> {
        self.review_queue_selected(project_id, super::scope::SegmentSelection::Inventory, None)
    }

    pub fn review_queue_selected(
        &self,
        project_id: &str,
        selection: super::scope::SegmentSelection,
        category: Option<crate::ResourceCategory>,
    ) -> Result<(usize, Vec<SegmentWithFile>)> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let scope_filter = match selection {
            super::scope::SegmentSelection::Inventory => "",
            super::scope::SegmentSelection::Task => {
                super::scope::validate_scope(&tx, project_id)?;
                "AND EXISTS(SELECT 1 FROM translation_scope_segments t WHERE t.project_id=s.project_id AND t.segment_id=s.id)"
            }
        };
        let total = tx.query_row(
            &format!(
                "SELECT COUNT(*) FROM segments s JOIN files f ON f.id=s.file_id
            WHERE s.project_id=?1 AND {REVIEW_FILTER} {scope_filter} AND (?2 IS NULL OR f.category=?2)"
            ),
            params![project_id, category.map(crate::ResourceCategory::as_str)],
            |row| row.get(0),
        )?;
        let items = {
            let mut stmt = tx.prepare(&format!("SELECT s.id,s.project_id,s.file_id,f.relative_path,s.key_path,
                s.source_kr,s.previous_kr,s.target_zh,s.speaker,s.context_info,s.stage,s.qa_passed,s.qa_issues,
                s.updated_at,f.is_locked,s.review_confirmed FROM segments s JOIN files f ON f.id=s.file_id
                WHERE s.project_id=?1 AND {REVIEW_FILTER} {scope_filter} AND (?2 IS NULL OR f.category=?2) ORDER BY f.relative_path,s.id LIMIT 100"))?;
            let rows = stmt.query_map(
                params![project_id, category.map(crate::ResourceCategory::as_str)],
                segment_from_row,
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.commit()?;
        Ok((total, items))
    }
}
