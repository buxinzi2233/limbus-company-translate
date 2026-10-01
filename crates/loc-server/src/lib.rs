pub mod embedded_assets;
pub mod routes;

use anyhow::Result;
use loc_core::Store;
use std::net::SocketAddr;
use tracing::info;

pub use routes::{create_router, create_scoped_router};

/// 启动 Axum HTTP 服务，同时提供 REST API 与内嵌 WebUI
pub async fn start_server(
    port: u16,
    open_browser: bool,
    store: Store,
    project_id: Option<String>,
) -> Result<()> {
    let selected = match project_id {
        Some(id) => {
            anyhow::ensure!(
                store.get_project(&id)?.is_some(),
                "Project {id} does not exist"
            );
            Some(id)
        }
        None => store.get_latest_project()?.map(|project| project.id),
    };
    let app = create_scoped_router(store, selected);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let url = format!("http://{}", addr);
    info!("🚀 LCT WebUI 态势大盘服务已启动: {}", url);
    println!("==================================================");
    println!("🚀 LCT (Limbus Company Translate) 态势大盘正在运行:");
    println!("👉 本地访问地址: {}", url);
    if open_browser {
        println!("🌐 正在调用系统默认浏览器打开大盘...");
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&url).spawn();
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", &url])
                .spawn();
        });
    }
    println!("   按 Ctrl+C 可停止服务");
    println!("==================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
