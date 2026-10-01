//! Bound protocol frames without replacing the official SDK's JSON-RPC codec.
use anyhow::{bail, Result};
use futures_util::StreamExt;
use rmcp::{
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::{async_rw::JsonRpcMessageCodec, sink_stream::SinkStreamTransport},
    ServiceExt,
};
use tokio_util::codec::{FramedRead, FramedWrite};

pub async fn serve(server: super::ProjectServer) -> Result<()> {
    let (sender, mut errors) = tokio::sync::oneshot::channel::<String>();
    let input = FramedRead::new(
        tokio::io::stdin(),
        JsonRpcMessageCodec::<ClientJsonRpcMessage>::new_with_max_length(1_048_576),
    )
    .scan(Some(sender), |sender, frame| {
        let message = match frame {
            Ok(message) => Some(message),
            Err(error) => {
                // Close the session on malformed or oversized frames, never skip them.
                if let Some(sender) = sender.take() {
                    let _receiver_closed =
                        sender.send(format!("Invalid MCP frame (maximum 1 MiB): {error}"));
                }
                None
            }
        };
        std::future::ready(message)
    });
    let output = FramedWrite::new(
        tokio::io::stdout(),
        JsonRpcMessageCodec::<ServerJsonRpcMessage>::new(),
    );
    let service = server.serve(SinkStreamTransport::new(output, input)).await;
    if let Ok(error) = errors.try_recv() {
        bail!(error);
    }
    service?.waiting().await?;
    if let Ok(error) = errors.try_recv() {
        bail!(error);
    }
    Ok(())
}
