use axum::{
    Router,
    extract::{Json, Multipart},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::io::AsyncWriteExt;

const UPLOADS_DIRECTORY: &str = "data";
const HEALTH_LOG_FILE: &str = "data/health.log";

#[derive(Serialize, Deserialize)]
struct HealthRequest {
    voltage: f32,
    timestamp: String,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::max())
        .init();

    // Create uploads directory
    if let Err(e) = std::fs::create_dir_all(UPLOADS_DIRECTORY) {
        log::error!("Failed to create uploads directory: {e}");
    }

    let app = Router::new()
        .route("/", get(root))
        .route("/upload", post(upload_file))
        .route("/health", post(health));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Hello, World!"
}

async fn upload_file(mut multipart: Multipart) -> StatusCode {
    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let filename = field.file_name().unwrap().to_string();
        log::info!("Received file: {filename}");
        let path = Path::new(UPLOADS_DIRECTORY).join(&filename);

        let mut file = tokio::fs::File::create(path).await.unwrap();
        while let Some(chunk) = field.chunk().await.unwrap() {
            file.write_all(&chunk).await.unwrap();
        }
    }
    StatusCode::OK
}

async fn health(Json(request): Json<HealthRequest>) -> StatusCode {
    log::info!("Got device battery voltage: {}", request.voltage);

    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(HEALTH_LOG_FILE)
        .await
        .unwrap();
    file.write_all(&format!("{},{}\n", request.timestamp, request.voltage).as_bytes())
        .await
        .unwrap();

    StatusCode::OK
}
