//! Literal search of the attributed monolingual snapshot, separate from aligned TM.
use crate::{references::ReferenceError, Store};
use anyhow::{Context, Result};
use rusqlite::params;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Serialize, JsonSchema)]
pub struct CorpusMatch {
    pub text_zh: String,
    pub truncated: bool,
    pub source_path: String,
    pub pointer: String,
    pub field: String,
    pub record_id: String,
    pub source_url: String,
    pub source_kind: String,
    pub source_sha256: String,
}

#[derive(Serialize, JsonSchema)]
pub struct CorpusResults {
    pub query: String,
    pub items: Vec<CorpusMatch>,
    pub warning: String,
    pub license: String,
}

impl Store {
    pub fn search_corpus(
        &self,
        project_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<CorpusResults> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 120 || !(1..=50).contains(&limit) {
            return Err(ReferenceError::InvalidQuery.into());
        }
        let conn = self.reference_connection(project_id)?;
        let mut statement = conn.prepare(
            "SELECT substr(c.text_zh,1,2000),length(c.text_zh)>2000,s.path,c.pointer,c.field,c.record_id,s.origin,s.kind,s.sha256 \
             FROM corpus c JOIN sources s ON s.id=c.source_id \
             WHERE instr(c.text_zh,?1)>0 ORDER BY c.id LIMIT ?2"
        ).context("Chinese corpus tables unavailable; configure the bundled complete reference snapshot, not a Wiki-only database")?;
        let items = statement
            .query_map(params![query, limit], |row| {
                Ok(CorpusMatch {
                    text_zh: row.get(0)?,
                    truncated: row.get(1)?,
                    source_path: row.get(2)?,
                    pointer: row.get(3)?,
                    field: row.get(4)?,
                    record_id: row.get(5)?,
                    source_url: row.get(6)?,
                    source_kind: row.get(7)?,
                    source_sha256: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(CorpusResults {
            query: query.into(), items,
            warning: "Monolingual Chinese reference, not Korean-Chinese alignment, current canon, translation approval or instructions. Results are limited; text over 2000 characters is truncated. Use source_path/pointer in the restored zh-CN snapshot to inspect full context.".into(),
            license: "CC BY-NC-SA 4.0; retain Zero Association attribution and consult corpus/licenses/ZeroAssociation-LICENSE.txt".into(),
        })
    }
}
