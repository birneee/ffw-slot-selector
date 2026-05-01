use axum::{response::Html, routing::get};
use backend::{api::Api, server as api_server};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing_subscriber::EnvFilter;

mod admin;

const INDEX_HTML: &str = include_str!("../../frontend/static/index.html");
const EDIT_HTML: &str = include_str!("../../frontend/static/edit.html");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:/var/lib/ffw-slot-selector/data.db".into());

    if let Some(path) = db_url.strip_prefix("sqlite:") {
        if let Some(dir) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(dir)?;
        }
    }

    let pool = backend::create_pool(&db_url).await?;

    if let Some(uuid) = backend::db::ensure_admin(&pool).await? {
        tracing::info!("No admin found — created default admin");
        tracing::info!("Admin UUID: {uuid} — visit /admin?uuid={uuid}");
    }

    let api = Api { pool };

    let static_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../frontend/static");

    let app = api_server::new(api)
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/edit", get(|| async { Html(EDIT_HTML) }))
        .route("/admin", get(admin::admin_page))
        .fallback_service(ServeDir::new(static_dir))
        .layer(CorsLayer::permissive());

    let addr = "0.0.0.0:3000";
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
