use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use loc_core::{Project, Store};
use serde_json::json;
use std::path::PathBuf;

use loc_cli::commands;

#[derive(Parser)]
#[command(name = "lct")]
#[command(author = "Antigravity & Hackimi Localization Team")]
#[command(version = "0.2.1")]
#[command(about = "Limbus Company Translate (LCT) - 边狱公司工业级本地化工程平台", long_about = None)]
struct Cli {
    /// 启用机器可读的 JSON 格式输出
    #[arg(long, global = true)]
    json: bool,

    /// 本地存储目录 (默认为当前目录的 .loc)
    #[arg(long, global = true, default_value = ".loc")]
    data_dir: PathBuf,

    /// Read-only Wiki reference database; requires an explicitly bound UI or MCP project.
    #[arg(long, global = true)]
    reference_db: Option<PathBuf>,

    /// Hash-verified historical reports, exposed through read-only MCP research tools.
    #[arg(long, global = true)]
    research_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Read the validated task scope for an explicit project.
    ScopeStatus {
        #[arg(long)]
        project_id: String,
    },
    /// Validate and persist a task scope without changing source or translations.
    ScopeImport {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        input: PathBuf,
    },
    /// Serve bounded project tools over MCP stdio; no human approval or file output tools.
    Mcp {
        #[arg(long)]
        project_id: String,
        /// Process-bound first-draft capability; omission makes this connection read-only.
        #[arg(long)]
        draft_permit: Option<PathBuf>,
    },
    /// Issue an exclusive source-pinned first-draft grant after explicit human authorization.
    DraftGrant {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Revoke a grant immediately for all processes holding its permit.
    DraftRevoke {
        #[arg(long)]
        project_id: String,
        #[arg(long)]
        grant_id: String,
    },
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
    /// Submit a QA-gated AI draft; never marks it as manually reviewed.
    Draft {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        draft_permit: PathBuf,
    },
    /// Restore an injection backup without overwriting unrelated edits.
    Restore {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        backup: PathBuf,
    },
    /// 毫秒级反向实机台词/词条全文搜索
    Search {
        /// 搜索关键词 (支持中韩双语片段、角色名)
        query: String,
        /// 最多返回结果条数
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// 启动本地 WebUI 态势大盘与 REST API 服务 (默认自动在默认浏览器中拉起大盘)
    Ui {
        #[arg(long)]
        project_id: Option<String>,
        /// 本地服务端口 (默认 3000)
        #[arg(short, long, default_value_t = 3000)]
        port: u16,
        /// 禁止自动打开浏览器
        #[arg(long)]
        no_open: bool,
    },
    /// 运行本地化门禁与语义一致性稽核 (支持 --json 诊断输出)
    Qa {
        /// 待检测中文文本
        text: String,
        /// 原文韩文文本 (用于比对占位符、数值与极性)
        #[arg(short, long, default_value = "")]
        kr: String,
    },
    /// 对中文文本执行确定性安全自动修复 (波浪号转半角 + 关键词空格补齐)
    Fix {
        /// 待修复的中文文本
        text: String,
    },
    /// 索引双语基准语料，全量反哺 TM 翻译记忆库
    Index {
        /// 韩文原版生肉目录 (如 references/baseline-KR)
        #[arg(long)]
        kr: PathBuf,
        /// 零协会中文全量目录 (如 references/LLC-2026092102 或 workspace/LLC_zh-CN)
        #[arg(long)]
        zh: PathBuf,
        /// 语料来源标识 (如 zero-association-20260921)
        #[arg(long, default_value = "zero-association")]
        origin: String,
        /// 记忆库优先级权重 (数字越大越优先，默认 100)
        #[arg(long, default_value_t = 100)]
        priority: i32,
    },
    /// 自动比对生肉与当前汉化，提取增量待译条目至项目数据库 (可导出工单 JSONL)
    Extract {
        /// 项目标识代号 (默认使用最新活跃项目)
        #[arg(long)]
        project_id: Option<String>,
        /// 韩文原版生肉目录 (如 Steam 客户端或 references/baseline-KR)
        #[arg(long)]
        kr: PathBuf,
        /// 当前中文工作区或基准目录 (如 workspace/LLC_zh-CN)
        #[arg(long)]
        zh: PathBuf,
        /// 可选：将增量工单导出为独立 JSONL 文件供 AI 或外部协作
        #[arg(long)]
        ticket: Option<PathBuf>,
        /// 是否自动基于 TM 记忆库为 100%/101% 匹配项预填译文 (默认开启)
        #[arg(long, default_value_t = true)]
        auto_fill_tm: bool,
    },
    /// 将翻译成果从工单文件或项目数据库安全注入回填至游戏 JSON 工作区
    Inject {
        #[arg(long, conflicts_with = "ticket")]
        project_id: Option<String>,
        /// 目标中文工作区目录 (如 workspace/LLC_zh-CN)
        #[arg(long)]
        workspace: PathBuf,
        /// 韩文模板目录 (当目标工作区不存在该文件时，从此目录复制生肉结构模板)
        #[arg(long)]
        kr: Option<PathBuf>,
        /// 工单 JSONL 路径 (若不指定，则直接从当前项目数据库中读取已翻译条目)
        #[arg(long)]
        ticket: Option<PathBuf>,
        /// 是否在注入前严格执行门禁检查 (若有 FATAL 级错误将拦截写入)
        #[arg(long, default_value_t = true)]
        qa_check: bool,
        /// 忽略门禁警告强行写入
        #[arg(long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::WARN)
        .try_init()
        .map_err(|error| anyhow::anyhow!("Initialize CLI diagnostics: {error}"))?;
    if matches!(cli.command, Commands::Mcp { .. }) {
        anyhow::ensure!(
            cli.data_dir.join("project.db").is_file(),
            "MCP requires an existing database: {}",
            cli.data_dir.join("project.db").display()
        );
    }
    std::fs::create_dir_all(&cli.data_dir)?;
    let db_path = cli.data_dir.join("project.db");
    let store = Store::open(&db_path)?;
    let store = match &cli.reference_db {
        Some(path) => {
            let project_id = match &cli.command {
                Commands::Mcp { project_id, .. }
                | Commands::Ui {
                    project_id: Some(project_id),
                    ..
                } => project_id,
                _ => anyhow::bail!("--reference-db requires mcp or ui with explicit --project-id"),
            };
            store.with_reference(project_id, path)?
        }
        None => store,
    };

    let research = match &cli.research_dir {
        Some(path) => {
            anyhow::ensure!(
                matches!(&cli.command, Commands::Mcp { .. }),
                "--research-dir requires mcp with explicit --project-id"
            );
            Some(loc_core::research::load_research(path)?)
        }
        None => None,
    };

    match cli.command {
        Commands::ScopeStatus { project_id } => {
            let scope = store
                .task_scope(&project_id)?
                .ok_or(loc_core::ScopeError::Missing(project_id))?;
            println!("{}", serde_json::to_string_pretty(&scope)?);
        }
        Commands::ScopeImport {
            project_id,
            name,
            input,
        } => {
            commands::scope::import_scope(&store, &project_id, &name, &input)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&store.task_scope(&project_id)?)?
            );
        }

        Commands::Mcp {
            project_id,
            draft_permit,
        } => {
            let permit = draft_permit
                .as_deref()
                .map(commands::draft_grants::read_permit)
                .transpose()?;
            loc_mcp::serve(store, project_id, permit, research).await?;
        }
        Commands::DraftGrant { input, output } => {
            commands::draft_grants::create_grant(&store, &input, &output)?;
        }
        Commands::DraftRevoke {
            project_id,
            grant_id,
        } => {
            store.revoke_draft_grant(&project_id, &grant_id)?;
            println!("{}", json!({"revoked":grant_id,"project_id":project_id}));
        }
        Commands::Init { id, name } => {
            let project = Project::new(&id, &name);
            store.upsert_project(&project)?;
            if cli.json {
                println!(
                    "{}",
                    json!({ "status": "ok", "project_id": id, "name": name })
                );
            } else {
                println!("✓ 本地化工程已成功初始化: [{}] {}", id, name);
                println!("  数据底座保存在: {}", db_path.display());
            }
        }
        Commands::Restore { workspace, backup } => {
            let summary = commands::run_restore(&workspace, &backup)?;
            store.record_restoration(&workspace.canonicalize()?, &backup, summary.restored_files + summary.removed_files)
                .context("Files were restored, but the database audit receipt failed; inspect the workspace before retrying")?;
            if cli.json {
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "恢复完成：还原 {} 个文件，删除本批新增的 {} 个文件",
                    summary.restored_files, summary.removed_files
                );
            }
        }
        Commands::Draft {
            input,
            draft_permit,
        } => {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(&input)?
                .take(1_048_577)
                .read_to_end(&mut bytes)?;
            anyhow::ensure!(
                bytes.len() <= 1_048_576,
                "Draft input exceeds 1 MiB: {}",
                input.display()
            );
            let submission: loc_qa::DraftSubmission = serde_json::from_slice(&bytes)?;
            let permit = commands::draft_grants::read_permit(&draft_permit)?;
            let item = loc_qa::submit_ai_draft(&store, &submission, Some(&permit))?;
            if cli.json {
                println!("{}", json!({"status":"ok","item":item}));
            } else {
                println!("AI 草稿已保存，QA 通过但尚未人工审校：{}", item.id);
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
            let project = store
                .get_latest_project()?
                .ok_or_else(|| anyhow::anyhow!("No active project; run lct init first"))?;
            let seg_results = store.search_segments_fts(&project.id, &query, limit)?;
            let tm_results = store.search_tm_fts(&query, limit)?;
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "query": query,
                        "segments": seg_results,
                        "tm_entries": tm_results,
                        "total_found": seg_results.len() + tm_results.len()
                    })
                );
            } else {
                println!(
                    "🔍 检索到 {} 条项目活跃词条，{} 条 TM 记忆条目 (关键词: \"{}\"):",
                    seg_results.len(),
                    tm_results.len(),
                    query
                );
                if !seg_results.is_empty() {
                    println!("\n【当前项目活跃词条 (Segments)】:");
                    for (i, r) in seg_results.iter().enumerate() {
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
                if !tm_results.is_empty() {
                    println!("\n【翻译记忆库对齐条目 (TM Entries)】:");
                    for (i, r) in tm_results.iter().enumerate() {
                        println!(
                            "  [{}] 来源: {} (权重: {}) | 文件范围: {}\n      KR: {}\n      ZH: {}\n      说话人: {:?}",
                            i + 1,
                            r.source_origin,
                            r.priority,
                            r.file_scope.as_deref().unwrap_or("*"),
                            r.source_kr,
                            r.target_zh,
                            r.speaker.as_deref().unwrap_or("-")
                        );
                    }
                }
            }
        }
        Commands::Ui {
            port,
            no_open,
            project_id,
        } => {
            loc_server::start_server(port, !no_open, store, project_id).await?;
        }
        Commands::Qa { text, kr } => {
            let issues = loc_qa::check_segment_pair(&kr, &text);
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "passed": issues.is_empty(),
                        "issue_count": issues.len(),
                        "issues": issues,
                    })
                );
            } else if issues.is_empty() {
                println!("✓ 门禁与语义稽核全部通过！0 违规项。");
            } else {
                println!("🚨 发现 {} 处潜在质量门禁或语义问题:", issues.len());
                for (i, issue) in issues.iter().enumerate() {
                    println!(
                        "  [{}] [规则: {}] [级别: {:?}]\n      说明: {}\n      建议修复: {:?}",
                        i + 1,
                        issue.rule_id,
                        issue.severity,
                        issue.message,
                        issue.suggested_fix.as_deref().unwrap_or("需人工/模型复核")
                    );
                }
            }
        }
        Commands::Fix { text } => {
            let fixed = loc_qa::auto_fix_target_zh(&text);
            if cli.json {
                println!("{}", json!({ "original": text, "fixed": fixed }));
            } else {
                println!("原文本: {}", text);
                println!("修复后: {}", fixed);
            }
        }
        Commands::Index {
            kr,
            zh,
            origin,
            priority,
        } => {
            if !kr.exists() {
                anyhow::bail!("韩文原版目录不存在: {}", kr.display());
            }
            if !zh.exists() {
                anyhow::bail!("中文译文目录不存在: {}", zh.display());
            }
            let summary = loc_tm::index_parallel_corpora(&kr, &zh, &origin, priority, &store)?;
            let total_in_db = store.get_tm_count()?;
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "status": "ok",
                        "scanned_files": summary.scanned_files,
                        "aligned_entries": summary.aligned_entries,
                        "inserted_entries": summary.inserted_entries,
                        "total_tm_entries": total_in_db,
                        "elapsed_ms": summary.elapsed_ms,
                    })
                );
            } else {
                println!("✓ 双语语料全量对齐与 TM 反哺完成！");
                println!("  扫描有效双语文件: {} 个", summary.scanned_files);
                println!("  本轮对齐记忆词条: {} 条", summary.aligned_entries);
                println!("  本轮新增记忆词条: {} 条", summary.inserted_entries);
                println!("  数据库当前 TM 总量: {} 条", total_in_db);
                println!(
                    "  耗时: {:.2}s ({} ms)",
                    summary.elapsed_ms as f64 / 1000.0,
                    summary.elapsed_ms
                );
            }
        }
        Commands::Extract {
            project_id,
            kr,
            zh,
            ticket,
            auto_fill_tm,
        } => {
            let summary = commands::run_extract(
                &store,
                project_id,
                &kr,
                &zh,
                ticket.as_deref(),
                auto_fill_tm,
            )?;
            if cli.json {
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("✓ 增量扫描与待译提取完成！");
                println!("  扫描文件总数: {} 个", summary.scanned_files);
                println!("  提取条目总数: {} 条", summary.total_segments);
                println!("  已汉化保留:   {} 条", summary.already_translated);
                println!("  TM 自动预填:  {} 条", summary.tm_exact_filled);
                println!("  待翻译生肉:   {} 条", summary.pending_untranslated);
                println!("  待人工复核:   {} 条", summary.needs_review);
                println!("  快照绑定记录: {} 条", summary.snapshot_bound_segments);
                if let Some(tp) = ticket {
                    println!("  工单文件已导出至: {}", tp.display());
                }
                println!(
                    "  耗时: {:.2}s ({} ms)",
                    summary.elapsed_ms as f64 / 1000.0,
                    summary.elapsed_ms
                );
            }
        }
        Commands::Inject {
            project_id,
            workspace,
            kr,
            ticket,
            qa_check,
            force,
        } => {
            let summary = commands::inject::run_scoped_inject(
                &store,
                &workspace,
                kr.as_deref(),
                ticket.as_deref(),
                qa_check,
                force,
                project_id.as_deref(),
            )?;
            if cli.json {
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("✓ 翻译成果已安全注入回填游戏 JSON 工作区！");
                println!("  修改更新文件数: {} 个", summary.modified_files);
                println!("  成功回填词条数: {} 条", summary.injected_segments);
                if summary.skipped_untranslated > 0 {
                    println!("  跳过未翻译词条: {} 条", summary.skipped_untranslated);
                }
                if summary.qa_violations > 0 {
                    println!("  门禁违规警告数: {} 项", summary.qa_violations);
                }
                println!(
                    "  耗时: {:.2}s ({} ms)",
                    summary.elapsed_ms as f64 / 1000.0,
                    summary.elapsed_ms
                );
            }
        }
    }

    Ok(())
}
