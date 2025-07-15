use axum::{
    Router,
    extract::Multipart,
    http::StatusCode,
    routing::{get, post},
};
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

const UPLOADS_DIRECTORY: &str = "uploads";

#[tokio::main]
async fn main() {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::max())
        .init();

    // Create uploads directory
    if let Err(e) = std::fs::create_dir_all(UPLOADS_DIRECTORY) {
        log::error!("Failed to create uploads directory: {e}");
    }

    // build our application with a single route
    let app = Router::new()
        .route("/", get(root))
        .route("/upload", post(upload_file))
        .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(
            10 * 1024 * 1024,
        ))
        .layer(tower_http::timeout::TimeoutLayer::new(
            std::time::Duration::from_secs(120),
        ));

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
