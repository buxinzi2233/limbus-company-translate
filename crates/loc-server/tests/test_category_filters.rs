#[path = "../../loc-core/tests/support/draft_grants.rs"]
mod draft_grants;
use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use loc_core::{FileRecord, ImportFile, Project, ScopeManifest, Segment, Store};
use loc_qa::{submit_ai_draft, DraftSubmission};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn get(app: &Router, uri: &str) -> Result<Value> {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::OK, "{uri}");
    Ok(serde_json::from_slice(
        &to_bytes(response.into_body(), 2_000_000).await?,
    )?)
}

#[tokio::test]
async fn category_filters_precede_pagination_and_preserve_scope_and_audit() -> Result<()> {
    let store = Store::open_in_memory()?;
    let project = Project {
        live_client_kr_dir: Some("/integration/kr".into()),
        workspace_zh_dir: Some("/integration/zh".into()),
        ..Project::new("domains", "Domain filtering integration")
    };
    let resources = [
        ("Skills-a1c10p2.json", "battle", 26),
        ("StoryData/S1017B.json", "story", 120),
        ("RPGSystem/rpg-loc-ui-common-a1c10p2.json", "system", 2),
    ];
    let files: Vec<_> = resources
        .iter()
        .map(|(path, category, count)| {
            let segments: Vec<_> = (0..*count)
                .map(|i| Segment::new(0, "domains", format!("/dataList/{i}/text"), "안녕하세요"))
                .collect();
            ImportFile {
                identities: segments
                    .iter()
                    .map(|s| (s.key_path.clone(), format!("identity:{}", s.key_path)))
                    .collect(),
                file: FileRecord::new("domains", *path, *category),
                segments,
            }
        })
        .collect();
    let rows = store.import_files(&project, &files)?;
    let excluded = rows
        .iter()
        .find(|r| r.relative_path == resources[0].0)
        .unwrap()
        .id;
    let manifest: ScopeManifest = serde_json::from_value(json!({
        "project_id": "domains",
        "files": resources.iter().map(|(path, _, count)| json!({"path":path,"sha256":"a".repeat(64),"selected_segments":if *path==resources[0].0 {*count-1} else {*count}})).collect::<Vec<_>>(),
        "segments": rows.iter().filter(|r| r.id!=excluded).map(|r| json!({"segment_id":r.id,"path":r.relative_path,"pointer":r.key_path,"source_identity":format!("identity:{}",r.key_path),"source_kr":r.source_kr})).collect::<Vec<_>>()
    }))?;
    let permit = draft_grants::grant(&store, "domains", rows.iter().map(|r| r.id).collect())?;
    for row in &rows {
        submit_ai_draft(
            &store,
            &DraftSubmission {
                project_id: "domains".into(),
                id: row.id,
                expected_updated_at: row.updated_at,
                target_zh: "你好".into(),
                producer: "domain-integration".into(),
            },
            Some(&permit),
        )?;
    }
    store.import_task_scope(
        &manifest,
        "Domain task",
        &"b".repeat(64),
        store.latest_source_import_id("domains")?,
    )?;
    let app = loc_server::create_router(store.clone());
    for (category, count) in [("battle", 26), ("story", 120), ("system", 2)] {
        let page = get(
            &app,
            &format!(
                "/api/segments?category={category}&stage=3&search=%E4%BD%A0%E5%A5%BD&page_size=25"
            ),
        )
        .await?;
        assert_eq!(page["total"], count);
        assert!(page["items"].as_array().unwrap().iter().all(|r| resources
            .iter()
            .any(|(path, c, _)| *c == category && r["relative_path"] == *path)));
        let queue = get(&app, &format!("/api/review-queue?category={category}")).await?;
        assert_eq!(queue["total"], count);
        assert_eq!(
            queue["items"].as_array().unwrap().len(),
            count.min(100) as usize
        );
    }
    let first = get(&app, "/api/segments?category=battle&page_size=25").await?;
    let second = get(&app, "/api/segments?category=battle&page_size=25&page=2").await?;
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert!(!first["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == second["items"][0]["id"]));
    for endpoint in ["segments", "review-queue"] {
        let filtered = get(
            &app,
            &format!("/api/{endpoint}?category=battle&selection=task"),
        )
        .await?;
        assert_eq!(filtered["total"], 25);
        assert!(filtered["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"] != excluded));
    }
    let empty = get(&app, "/api/segments?category=story&stage=0").await?;
    assert_eq!(empty["total"], 0);
    let audit = get(&app, "/api/audit?category=battle").await?;
    assert_eq!(audit["events"].as_array().unwrap().len(), 20);
    let cursor = audit["next_events"].as_i64().unwrap();
    let older = get(&app, &format!("/api/audit?category=battle&before={cursor}")).await?;
    assert_eq!(older["events"].as_array().unwrap().len(), 6);
    assert!(older["next_events"].is_null());
    for event in audit["events"]
        .as_array()
        .unwrap()
        .iter()
        .chain(older["events"].as_array().unwrap())
    {
        assert_eq!(
            store
                .get_segment("domains", event["segment_id"].as_i64().unwrap())?
                .relative_path,
            resources[0].0
        );
    }
    let all = get(&app, "/api/audit").await?;
    assert_eq!(all["imports"], audit["imports"]);
    assert_eq!(all["corpora"], audit["corpora"]);
    assert_eq!(all["operations"], audit["operations"]);
    assert_eq!(get(&app, "/api/segments").await?["total"], 148);
    for endpoint in ["segments", "review-queue", "audit"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/{endpoint}?category=invalid"))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    assert!(rows
        .iter()
        .all(|r| !store.get_segment("domains", r.id).unwrap().review_confirmed));
    Ok(())
}
