use axum::{Json, Router, routing::get};
use shared::Health;

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".into(),
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/api/health", get(health));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("bind 127.0.0.1:8080");
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("serve");
}
