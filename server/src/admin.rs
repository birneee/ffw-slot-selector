use axum::response::{Html, IntoResponse};

const ADMIN_HTML: &str = include_str!("../../frontend/static/admin.html");

pub async fn admin_page() -> impl IntoResponse {
    Html(ADMIN_HTML)
}
