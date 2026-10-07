use axum::{Json, Router, routing::get};
use agora_shared::Health;

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".into(),
    })
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/api/health", get(health));

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.expect("serve");
}
