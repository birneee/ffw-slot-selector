use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
};
use backend::{api::Api, server as api_server};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
}

mod admin;

const INDEX_HTML: &str = include_str!("../../frontend/static/index.html");
const EDIT_HTML: &str = include_str!("../../frontend/static/edit.html");
const STYLE_CSS: &str = include_str!("../../frontend/static/style.css");
const FRONTEND_JS: &str = include_str!("../../frontend/static/frontend.js");
const FRONTEND_WASM: &[u8] = include_bytes!("../../frontend/static/frontend_bg.wasm");
const WAPPEN_PNG: &[u8] = include_bytes!("../../frontend/static/wappen.png");
const LOGO_PNG: &[u8] = include_bytes!("../../frontend/static/150-jahre-logo.png");

async fn style_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], STYLE_CSS)
}

async fn frontend_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], FRONTEND_JS)
}

async fn frontend_wasm() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/wasm")], FRONTEND_WASM)
}

async fn wappen_png() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/png")], WAPPEN_PNG)
}

async fn logo_png() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/png")], LOGO_PNG)
}

async fn token_page(Path(token): Path<String>) -> impl IntoResponse {
    if token.len() == 22 {
        Html(INDEX_HTML).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

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
        tracing::info!("No admin found — created default admin with UUID: {uuid}");
    }

    let admin_uuid = backend::db::get_admin_uuid(&pool).await?;
    let admin_b64 = URL_SAFE_NO_PAD.encode(uuid::Uuid::parse_str(&admin_uuid)?.as_bytes());
    tracing::info!("Admin panel: /admin?uuid={admin_b64}");

    let api = Api { pool };

    let app = api_server::new(api)
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/edit", get(|| async { Html(EDIT_HTML) }))
        .route("/admin", get(admin::admin_page))
        .route("/style.css", get(style_css))
        .route("/frontend.js", get(frontend_js))
        .route("/frontend_bg.wasm", get(frontend_wasm))
        .route("/wappen.png", get(wappen_png))
        .route("/150-jahre-logo.png", get(logo_png))
        .route("/{token}", get(token_page));

    let addr = format!("0.0.0.0:{}", args.port);
    tracing::info!("listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
