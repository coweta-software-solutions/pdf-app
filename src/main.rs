use std::{net::SocketAddr, sync::Arc};

use pdf_tools_server::{app, read_env_u16, read_env_usize, AppResult, AppState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> AppResult<()> {
    let state = Arc::new(AppState::from_env()?);
    let max_upload_mb = read_env_usize("MAX_UPLOAD_MB", 100);
    let port = read_env_u16("PORT", 3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app(state, max_upload_mb)).await?;
    Ok(())
}
