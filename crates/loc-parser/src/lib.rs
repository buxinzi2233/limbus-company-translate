pub mod extractor;
pub mod hangul;
pub mod injector;
pub mod pointer;

pub use extractor::{extract_segments_from_value, ExtractedItem};
pub use hangul::contains_hangul;
pub use injector::{inject_translations_into_value, serialize_to_game_json};
pub use pointer::{get_at_pointer, set_at_pointer};

pub mod assets;
pub use assets::{inventory_korean_assets, normalize_asset_path};

pub mod identity;
pub use identity::source_identities;

pub mod json;
pub use json::parse_game_json;
