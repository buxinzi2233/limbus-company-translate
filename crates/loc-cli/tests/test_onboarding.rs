use anyhow::{ensure, Context, Result};
use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, process::Command, time::Duration};
use tempfile::tempdir;

#[derive(Deserialize)]
struct Server {
    command: String,
    args: Vec<String>,
}

#[derive(Deserialize)]
struct ClientConfig {
    #[serde(rename = "mcpServers")]
    servers: BTreeMap<String, Server>,
}

fn request(name: &str, arguments: Value) -> CallToolRequestParams {
    CallToolRequestParams::new(name.to_owned())
        .with_arguments(arguments.as_object().unwrap().clone())
}

#[tokio::test]
async fn new_user_configuration_connects_to_real_bundled_resources() -> Result<()> {
    tokio::time::timeout(Duration::from_secs(120), async {
        let root = tempdir()?;
        let workspace = root.path().join("new user workspace");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let args = [
            "--binary",
            env!("CARGO_BIN_EXE_lct"),
            "--workspace",
            workspace.to_str().unwrap(),
            "--project-id",
            "onboarding",
            "--name",
            "New project",
            "--port",
            "33001",
        ];
        let output = Command::new("python3")
            .arg(source.join("scripts/setup-workspace.py"))
            .args(args)
            .output()?;
        ensure!(
            output.status.success(),
            "Setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let config_bytes = std::fs::read(workspace.join("mcp.json"))?;
        let config: ClientConfig = serde_json::from_slice(&config_bytes)?;
        let server = config
            .servers
            .get("lct")
            .context("Missing generated lct configuration")?;
        assert!(PathBuf::from(&server.command).is_absolute());
        assert!(!server.args.iter().any(|arg| arg == "--draft-permit"));
        assert!(workspace.join("AGENTS.md").is_file());
        assert!(workspace.join("start-ui.sh").is_file());
        let denied = Command::new("python3")
            .arg(source.join("scripts/setup-workspace.py"))
            .args(args)
            .output()?;
        assert!(!denied.status.success());
        assert!(String::from_utf8_lossy(&denied.stderr).contains("Refusing to overwrite"));
        assert_eq!(std::fs::read(workspace.join("mcp.json"))?, config_bytes);
        let transport = TokioChildProcess::new(
            tokio::process::Command::new(&server.command).configure(|c| {
                c.current_dir(root.path()).args(&server.args);
            }),
        )?;
        let client = ().serve(transport).await?;
        let tools = client.list_all_tools().await?;
        assert_eq!(tools.len(), 13);
        for name in [
            "list_research",
            "search_research",
            "read_research",
            "search_corpus",
        ] {
            let tool = tools
                .iter()
                .find(|t| t.name == name)
                .context("Missing read tool")?;
            assert_eq!(
                tool.annotations.as_ref().unwrap().read_only_hint,
                Some(true)
            );
        }
        let status = client
            .call_tool(request(
                "project_status",
                json!({"project_id":"onboarding"}),
            ))
            .await?;
        assert_eq!(status.structured_content.unwrap()["total"], 0);
        let page = client
            .call_tool(request(
                "list_research",
                json!({"project_id":"onboarding","offset":0,"limit":50}),
            ))
            .await?;
        let page = page.structured_content.unwrap();
        assert_eq!(page["total"], 65);
        assert_eq!(page["items"].as_array().unwrap().len(), 50);
        let tail = client
            .call_tool(request(
                "list_research",
                json!({"project_id":"onboarding","offset":50,"limit":50}),
            ))
            .await?;
        assert_eq!(
            tail.structured_content.unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            15
        );
        let found = client
            .call_tool(request(
                "search_research",
                json!({"project_id":"onboarding","query":"文风","limit":5}),
            ))
            .await?;
        assert!(found.structured_content.unwrap()["total"].as_u64().unwrap() > 0);
        let document = client
            .call_tool(request(
                "read_research",
                json!({"project_id":"onboarding","document_id":page["items"][0]["id"]}),
            ))
            .await?;
        let document = document.structured_content.unwrap();
        assert_eq!(document["metadata"]["status"], "historical_unverified");
        assert!(document["content"]
            .as_str()
            .unwrap()
            .starts_with("> 历史研究资料"));
        assert!(!document["content"].as_str().unwrap().contains("/home/"));
        let forbidden = client
            .call_tool(request(
                "read_research",
                json!({"project_id":"onboarding","document_id":"../README.md"}),
            ))
            .await?;
        assert_eq!(forbidden.structured_content.unwrap()["code"], "not_found");
        let foreign = client
            .call_tool(request(
                "list_research",
                json!({"project_id":"foreign","offset":0,"limit":5}),
            ))
            .await?;
        assert_eq!(
            foreign.structured_content.unwrap()["code"],
            "project_mismatch"
        );
        let wiki = client
            .call_tool(request(
                "search_references",
                json!({"project_id":"onboarding","query":"呼吸法","limit":5}),
            ))
            .await?;
        assert!(!wiki.structured_content.unwrap()["terms"]
            .as_array()
            .unwrap()
            .is_empty());
        let chinese = client
            .call_tool(request(
                "search_corpus",
                json!({"project_id":"onboarding","query":"经理","limit":5}),
            ))
            .await?;
        let chinese = chinese.structured_content.unwrap();
        assert!(!chinese["items"].as_array().unwrap().is_empty());
        for item in chinese["items"].as_array().unwrap() {
            assert!(item["text_zh"].as_str().unwrap().contains("经理"));
            assert!(item["source_url"]
                .as_str()
                .unwrap()
                .starts_with("https://github.com/"));
            assert_eq!(item["source_sha256"].as_str().unwrap().len(), 64);
        }
        for name in ["search_corpus", "search_research"] {
            let invalid = client
                .call_tool(request(
                    name,
                    json!({"project_id":"onboarding","query":" ","limit":5}),
                ))
                .await?;
            assert_eq!(invalid.structured_content.unwrap()["code"], "invalid_input");
        }
        client.cancel().await?;
        Ok(())
    })
    .await
    .context("New-user integration exceeded 120 seconds")?
}

#[test]
fn research_loader_rejects_corruption_missing_files_and_escaping_paths() -> Result<()> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../research");
    let library = loc_core::research::load_research(&source)?;
    assert_eq!(
        loc_core::research::list_research(&library, 0, 50)?.total,
        65
    );
    let root = tempdir()?;
    let mut catalog: Value = serde_json::from_slice(&std::fs::read(source.join("catalog.json"))?)?;
    let original = catalog["documents"][0].clone();
    catalog["documents"] = json!([original]);
    let id = catalog["documents"][0]["id"].as_str().unwrap().to_owned();
    let report = root.path().join(&id);
    std::fs::create_dir_all(report.parent().unwrap())?;
    std::fs::write(
        root.path().join("catalog.json"),
        serde_json::to_vec(&catalog)?,
    )?;
    assert!(loc_core::research::load_research(root.path())
        .err()
        .unwrap()
        .to_string()
        .contains("Resolve research document"));
    std::fs::copy(source.join(&id), &report)?;
    loc_core::research::load_research(root.path())?;
    std::fs::write(&report, "corrupted real report")?;
    assert!(loc_core::research::load_research(root.path())
        .err()
        .unwrap()
        .to_string()
        .contains("SHA-256 mismatch"));
    catalog["documents"][0]["id"] = json!("../outside.md");
    std::fs::write(
        root.path().join("catalog.json"),
        serde_json::to_vec(&catalog)?,
    )?;
    assert!(loc_core::research::load_research(root.path())
        .err()
        .unwrap()
        .to_string()
        .contains("Unsafe research document"));
    #[cfg(unix)]
    {
        catalog["documents"][0]["id"] = json!("escape.md");
        std::fs::write(
            root.path().join("catalog.json"),
            serde_json::to_vec(&catalog)?,
        )?;
        std::os::unix::fs::symlink(
            source.join(&id).canonicalize()?,
            root.path().join("escape.md"),
        )?;
        assert!(loc_core::research::load_research(root.path())
            .err()
            .unwrap()
            .to_string()
            .contains("escapes configured directory"));
    }
    Ok(())
}
