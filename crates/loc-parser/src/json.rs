use serde_json::Value;

/// Decode JSON with an optional UTF-8 BOM without rewriting the source bytes.
pub fn parse_game_json(bytes: &[u8]) -> serde_json::Result<Value> {
    serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes))
}
