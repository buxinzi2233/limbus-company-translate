use anyhow::Result;
use loc_core::Store;
use loc_cli::commands::{run_extract, run_inject};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_extract_and_inject_pipeline() -> Result<()> {
    let tmp = tempdir()?;
    let db_path = tmp.path().join("test_pipe.db");
    let store = Store::open(&db_path)?;

    let kr_dir = tmp.path().join("kr");
    let zh_dir = tmp.path().join("zh");
    let ticket_path = tmp.path().join("ticket.jsonl");

    fs::create_dir_all(kr_dir.join("StoryData"))?;
    fs::create_dir_all(&zh_dir)?;

    let sample_kr_story = json!({
        "dataList": [
            {
                "id": 1,
                "model": "Dante",
                "teller": "단테",
                "content": "우리는 마침내 도착했다."
            },
            {
                "id": 2,
                "model": "Faust",
                "teller": "파우스트",
                "content": "이곳이 바로 종착점입니다."
            }
        ]
    });

    fs::write(
        kr_dir.join("StoryData/S999A.json"),
        serde_json::to_string_pretty(&sample_kr_story)?,
    )?;

    // 1. 运行 extract 提取生肉
    let extract_summary = run_extract(
        &store,
        Some("test-pipe-proj".to_string()),
        &kr_dir,
        &zh_dir,
        Some(&ticket_path),
        true,
    )?;

    assert_eq!(extract_summary.scanned_files, 1);
    assert_eq!(extract_summary.total_segments, 2);
    assert_eq!(extract_summary.pending_untranslated, 2);
    assert!(ticket_path.exists());

    // 2. 模拟外部 AI/人工处理工单: 填入中文翻译
    let ticket_content = fs::read_to_string(&ticket_path)?;
    let mut updated_lines = Vec::new();
    for line in ticket_content.lines() {
        let mut item: serde_json::Value = serde_json::from_str(line)?;
        let kr = item["source_kr"].as_str().unwrap();
        let trans = if kr.contains("도착") {
            "我们终于抵达了。"
        } else {
            "这里正是终点。"
        };
        item["target_zh"] = serde_json::Value::String(trans.to_string());
        updated_lines.push(serde_json::to_string(&item)?);
    }
    let updated_ticket_path = tmp.path().join("ticket_translated.jsonl");
    fs::write(&updated_ticket_path, updated_lines.join("\n"))?;

    // 3. 运行 inject 回填翻译
    let inject_summary = run_inject(
        &store,
        &zh_dir,
        Some(&kr_dir),
        Some(&updated_ticket_path),
        true,
        false,
    )?;

    assert_eq!(inject_summary.modified_files, 1);
    assert_eq!(inject_summary.injected_segments, 2);
    assert_eq!(inject_summary.qa_violations, 0);

    // 4. 验证回填后的中文 JSON 文件完整性与正确性
    let injected_file = zh_dir.join("StoryData/S999A.json");
    assert!(injected_file.exists());
    let injected_content = fs::read_to_string(&injected_file)?;
    let injected_val: serde_json::Value = serde_json::from_str(&injected_content)?;

    assert_eq!(
        injected_val["dataList"][0]["content"].as_str(),
        Some("我们终于抵达了。")
    );
    assert_eq!(
        injected_val["dataList"][0]["teller"].as_str(),
        Some("단테")
    );
    assert_eq!(
        injected_val["dataList"][1]["content"].as_str(),
        Some("这里正是终点。")
    );

    // 5. 验证门禁熔断：如果工单中包含全角波浪号，inject 必须主动硬拦截
    let bad_ticket_path = tmp.path().join("bad_ticket.jsonl");
    let bad_item = json!({
        "file": "StoryData/S999A.json",
        "pointer": "/dataList/0/content",
        "source_kr": "우리는 마침내 도착했다.",
        "target_zh": "我们终于抵达了～" // 致命错误：全角波浪号
    });
    fs::write(&bad_ticket_path, serde_json::to_string(&bad_item)?)?;

    let fail_res = run_inject(
        &store,
        &zh_dir,
        Some(&kr_dir),
        Some(&bad_ticket_path),
        true,
        false,
    );
    assert!(fail_res.is_err(), "包含全角波浪号时必须被门禁拦截！");

    Ok(())
}
