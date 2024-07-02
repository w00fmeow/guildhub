use crate::modules::app::app::App;
use actix_web::HttpServer;
use std::sync::Arc;
use tracing::info;

pub mod configuration;
pub mod libs;
pub mod modules;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let app: Arc<App> = Arc::new(App::new().await);

    let app_ref = app.clone();

    info!(
        "Server is starting on port: {}",
        &app_ref.configuration.app_port
    );

    HttpServer::new(move || App::get_actix_app(app_ref.clone()))
        .bind(("127.0.0.1", app.configuration.app_port))?
        .run()
        .await
}
