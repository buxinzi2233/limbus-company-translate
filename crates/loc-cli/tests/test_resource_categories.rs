#[path = "../../loc-core/tests/support/draft_grants.rs"]
mod draft_grants;
use anyhow::Result;
use loc_cli::commands::run_extract;
use loc_core::{resource_category, SaveTranslation, Store};
use std::fs;
use tempfile::tempdir;

#[test]
fn extraction_and_legacy_reopen_share_categories_without_reimporting() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir_all(kr.join("RPGSystem"))?;
    fs::create_dir(&zh)?;
    let fixture = include_str!("../../loc-parser/tests/fixtures/rpg-current.json");
    fs::write(
        kr.join("RPGSystem/rpg-loc-dialogue-floor-4-b.json"),
        fixture,
    )?;
    let database = root.path().join("project.db");
    let store = Store::open(&database)?;
    let imported = run_extract(&store, Some("category-test".into()), &kr, &zh, None, false)?;
    assert_eq!(store.get_files("category-test")?[0].category, "story");
    let row = store
        .query_segments_paged("category-test", None, None, None, 0, 1)?
        .1
        .remove(0);
    let permit = draft_grants::grant(&store, "category-test", vec![row.id])?;
    store.save_ai_draft(
        &SaveTranslation {
            project_id: "category-test",
            id: row.id,
            expected_updated_at: row.updated_at,
            target_zh: "化妆品间有一枚不合时宜的小钉子。",
        },
        "category-integration",
        Some(&permit),
    )?;
    let before = serde_json::to_value(
        store
            .query_segments_paged("category-test", None, None, None, 0, 1000)?
            .1,
    )?;
    let source_import = store.latest_source_import_id("category-test")?;
    let file_before = store.get_files("category-test")?.remove(0);
    // Recreate the persisted category and schema version produced by the old extractor.
    store
        .raw_connection()
        .lock()
        .unwrap()
        .execute_batch("UPDATE files SET category='system'; PRAGMA user_version=1;")?;
    drop(store);
    for _ in 0..2 {
        let reopened = Store::open(&database)?;
        let file = reopened.get_files("category-test")?.remove(0);
        assert_eq!(file.category, "story");
        assert_eq!(file.id, file_before.id);
        assert_eq!(file.updated_at, file_before.updated_at);
        assert_eq!(
            reopened.latest_source_import_id("category-test")?,
            source_import
        );
        assert_eq!(
            serde_json::to_value(
                reopened
                    .query_segments_paged("category-test", None, None, None, 0, 1000)?
                    .1
            )?,
            before
        );
        let stats = reopened.get_project_stats("category-test")?;
        assert_eq!(stats.total_segments, imported.total_segments as i64);
        assert_eq!(stats.categories["story"].total, stats.total_segments);
        assert_eq!(stats.translated_segments, 1);
        assert_eq!(stats.pending_review_segments, 1);
        assert_eq!(stats.reviewed_segments, 0);
        let events: i64 = reopened.raw_connection().lock().unwrap().query_row(
            "SELECT COUNT(*) FROM translation_events",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(events, 1);
    }
    Ok(())
}

#[test]
fn resource_families_include_rpg_narrative_and_combat_metadata() {
    for (path, expected) in [
        ("StoryData/S1017B.json", "story"),
        ("RPGSystem/rpg-loc-dialogue-choice-floor-b1-b.json", "story"),
        ("RPGSystem/rpg-loc-narration-floor-3-b.json", "story"),
        ("RPGSystem/rpg-loc-quest-floor-4-b.json", "story"),
        ("RPGSystem\\rpg-loc-dialogue-floor-4-b.json", "story"),
        ("AbnormalityGuides-a1c10p2.json", "story"),
        ("PersonalityVoiceDlg/Voice.json", "story"),
        ("EGOVoiceDig/Voice.json", "story"),
        ("Skills_Abnormality-a1c10p2.json", "battle"),
        ("Bufs.json", "battle"),
        ("Passives_Abnormality-a1c10p2.json", "battle"),
        ("BattleKeywords-a1c10p2.json", "battle"),
        ("BattleSpeechBubbleDlg-a1c10p2.json", "battle"),
        ("BattleResultHint-a1c10p2.json", "battle"),
        ("PanicInfo-a1c10p2.json", "battle"),
        ("Enemies-a1c10p2.json", "battle"),
        ("Assist-a1c10p2.json", "battle"),
        ("RPGSystem/rpg-loc-item-common-a1c10p2.json", "system"),
        ("RPGSystem/rpg-loc-ui-common-a1c10p2.json", "system"),
        ("RPGSystem/rpg-loc-npc-floor-1-b.json", "system"),
        ("StoryTheaterUIText.json", "system"),
        ("StoryDatabase.json", "system"),
    ] {
        assert_eq!(resource_category(path), expected, "{path}");
    }
}
