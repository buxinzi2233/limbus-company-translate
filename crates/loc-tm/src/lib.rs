pub mod indexer;
pub mod matcher;

pub use indexer::{index_parallel_corpora, IndexSummary};
pub use matcher::{hash_text, match_segment, MatchCandidate, MatchTier};
