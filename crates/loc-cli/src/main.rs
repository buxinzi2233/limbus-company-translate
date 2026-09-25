use anyhow::Result;
use clap::{Parser, Subcommand};
use loc_core::{Project, Store};
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "loc")]
#[command(author = "Antigravity & Hackimi Localization Team")]
#[command(version = "0.1.0")]
#[command(about = "边狱公司工业级本地化工程平台 (loc-engine)", long_about = None)]
struct Cli {
    /// 启用机器可读的 JSON 格式输出
    #[arg(long, global = true)]
    json: bool,

    /// 本地存储目录 (默认为当前目录的 .loc)
    #[arg(long, global = true, default_value = ".loc")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化新的本地化工作区
    Init {
        /// 项目代号标识 (如 limbus-s8)
        id: String,
        /// 项目人类可读名称
        #[arg(short, long, default_value = "边狱公司新赛季汉化")]
        name: String,
    },
    /// 查看当前汉化进度与大盘状态
    Status,
    /// 毫秒级反向实机台词/词条全文搜索
    Search {
        /// 搜索关键词 (支持中韩双语片段、角色名)
        query: String,
        /// 最多返回结果条数
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    std::fs::create_dir_all(&cli.data_dir)?;
    let db_path = cli.data_dir.join("project.db");
    let store = Store::open(&db_path)?;

    match cli.command {
        Commands::Init { id, name } => {
            let project = Project::new(&id, &name);
            store.upsert_project(&project)?;
            if cli.json {
                println!("{}", json!({ "status": "ok", "project_id": id, "name": name }));
            } else {
                println!("✓ 本地化工程已成功初始化: [{}] {}", id, name);
                println!("  数据底座保存在: {}", db_path.display());
            }
        }
        Commands::Status => {
            let project = store.get_latest_project()?;
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "project": project,
                        "engine_version": "0.1.0",
                        "status": "ready"
                    })
                );
            } else {
                println!("==================================================");
                println!("  边狱公司本地化工程平台 (loc-engine) 状态报告");
                println!("==================================================");
                if let Some(p) = project {
                    println!("当前活跃项目: [{}] {}", p.id, p.name);
                } else {
                    println!("当前尚未初始化项目。运行 `loc init <id>` 初始化。");
                }
            }
        }
        Commands::Search { query, limit } => {
            let results = store.search_segments_fts(&query, limit)?;
            if cli.json {
                println!("{}", json!({ "query": query, "total": results.len(), "results": results }));
            } else {
                println!("🔍 检索到 {} 条匹配结果 (关键词: \"{}\"):", results.len(), query);
                for (i, r) in results.iter().enumerate() {
                    println!(
                        "  [{}] 文件: {} | 路径: {}\n      KR: {}\n      ZH: {}\n      说话人: {:?}",
                        i + 1,
                        r.relative_path,
                        r.key_path,
                        r.source_kr,
                        r.target_zh.as_deref().unwrap_or("<未翻译>"),
                        r.speaker.as_deref().unwrap_or("-")
                    );
                }
            }
        }
    }

    Ok(())
}
