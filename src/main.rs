use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Clone)]
struct AppState {
    s3: Client,
    bucket: String,
    event_token: String,
}

#[derive(Deserialize)]
struct UploadRequest {
    file_name: String,
    content_type: String,
    size: u64,
}
#[derive(Serialize)]
struct UploadResponse {
    upload_url: String,
    object_key: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //Loads environment variables from a .env file if it exists
    dotenvy::dotenv().ok();

    // Load the AWS SDK configuration
    let sdk_config = aws_config::defaults(BehaviorVersion::latest()).load().await;

    // Create an S3 client with the loaded configuration
    let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
        .endpoint_url(env::var("AWS_ENDPOINT_URL")?)
        .force_path_style(false)
        .build();

    let state = AppState {
        s3: Client::from_conf(s3_config),
        bucket: env::var("S3_BUCKET")?,
        event_token: env::var("EVENT_TOKEN")?,
    };

    let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
        .endpoint_url(env::var("AWS_ENDPOINT_URL")?)
        .force_path_style(false)
        .build();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()?;

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;

    println!("Server running at http://localhost:{}", port);

    let app = Router::new().route("/", get(index)).with_state(state);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn create_upload_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UploadRequest>,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    todo!()
}
