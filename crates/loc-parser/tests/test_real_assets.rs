use anyhow::Result;
use loc_parser::{
    extract_segments_from_value, inject_translations_into_value, serialize_to_game_json,
};
use serde_json::Value;

// Untranslated excerpts from the verified 2026-09-26 client snapshot.
// These fixtures never read a personal translation workspace.
#[test]
fn current_client_display_fields_round_trip_without_metadata_changes() -> Result<()> {
    for raw in [
        include_str!("fixtures/story-current.json"),
        include_str!("fixtures/rpg-current.json"),
    ] {
        let parsed: Value = serde_json::from_str(raw)?;
        let extracted = extract_segments_from_value(&parsed)?;
        assert!(!extracted.is_empty());
        let first = &extracted[0];
        let mut output = parsed.clone();
        inject_translations_into_value(&mut output, &[(first.pointer.as_str(), "测试文本 ")])?;
        let serialized = serialize_to_game_json(&output)?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual.pointer(&first.pointer).unwrap(), "测试文本 ");
        *output.pointer_mut(&first.pointer).unwrap() = Value::String(first.source_kr.clone());
        assert_eq!(output, parsed);
    }
    Ok(())
}
