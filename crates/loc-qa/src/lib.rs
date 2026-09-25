pub mod checker;
pub mod fixer;
pub mod issue;
pub mod rules;

pub use checker::check_segment_pair;
pub use fixer::auto_fix_target_zh;
pub use issue::{Issue, IssueSeverity};
