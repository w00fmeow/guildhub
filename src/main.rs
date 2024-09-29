use crate::modules::app::app::App;
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

pub mod configuration;
pub mod libs;
pub mod modules;

#[tokio::main]
async fn main() -> Result<()> {
    let app: Arc<App> = Arc::new(App::new().await);

    app.run_migrations().await?;

    let listener = tokio::net::TcpListener::bind(format!(
        "0.0.0.0:{}",
        app.configuration.app_port
    ))
    .await?;

    info!("Server is starting on port: {}", &app.configuration.app_port);

    let router = App::get_app_router(app);

    axum::serve(listener, router).await?;

    Ok(())
}
