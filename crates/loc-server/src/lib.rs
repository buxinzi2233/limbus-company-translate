pub mod embedded_assets;
pub mod routes;

use anyhow::Result;
use loc_core::Store;
use std::net::SocketAddr;
use tracing::info;

pub use routes::create_router;

/// 启动 Axum HTTP 服务，同时提供 REST API 与内嵌 WebUI
pub async fn start_server(port: u16, store: Store) -> Result<()> {
    let app = create_router(store);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("🚀 loc-engine WebUI 服务已启动: http://{}", addr);
    println!("==================================================");
    println!("🚀 边狱公司本地化工作台 (WebUI) 正在运行:");
    println!("👉 本地访问地址: http://{}", addr);
    println!("   按 Ctrl+C 可停止服务");
    println!("==================================================");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
