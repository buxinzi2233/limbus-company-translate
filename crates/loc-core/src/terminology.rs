//! Provenance-bearing name links; machine identity checks are not human approval.
use crate::Store;
use anyhow::{ensure, Result};
use rusqlite::OptionalExtension;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct TermLink {
    pub keyword_id: String,
    pub source_kr: String,
    pub target_zh: String,
    pub wiki_url: String,
    pub wiki_revision: i64,
    pub kr_path: String,
    pub kr_pointer: String,
    pub kr_sha256: String,
    pub zh_path: String,
    pub zh_pointer: String,
    pub zh_sha256: String,
    pub zero_release_url: String,
    pub source_manifest_url: String,
    pub zero_license_url: String,
    pub wiki_license_url: String,
    pub attribution: String,
    pub verification: String,
    pub review_status: String,
}

impl Store {
    pub fn term_links(&self, project_id: &str) -> Result<Vec<TermLink>> {
        if self.reference.is_none() {
            return Ok(Vec::new());
        }
        let conn = self.reference_connection(project_id)?;
        let version: Option<String> = conn
            .query_row(
                "SELECT value FROM metadata WHERE key='term_links_version'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let Some(version) = version else {
            return Ok(Vec::new());
        };
        ensure!(
            version == "1",
            "Unsupported term link schema version {version}"
        );
        let mut statement = conn.prepare("SELECT keyword_id,source_kr,target_zh,evidence_json,review_status FROM term_links ORDER BY keyword_id")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        let mut links = Vec::new();
        for row in rows {
            let (id, kr, zh, evidence, state) = row?;
            let link: TermLink = serde_json::from_str(&evidence)?;
            ensure!(
                link.keyword_id == id
                    && link.source_kr == kr
                    && link.target_zh == zh
                    && link.review_status == state,
                "Term link evidence does not match indexed fields for {id}"
            );
            ensure!(
                state == "unreviewed"
                    && link.verification == "exact_unique_id_and_wiki_name_checked",
                "Unsupported term verification state for {id}"
            );
            ensure!(
                !kr.is_empty()
                    && !zh.is_empty()
                    && !id.is_empty()
                    && link.kr_sha256.len() == 64
                    && link.zh_sha256.len() == 64,
                "Incomplete term provenance for {id}"
            );
            links.push(link);
        }
        Ok(links)
    }
}
