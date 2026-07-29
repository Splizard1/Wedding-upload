use aws_config::BehaviorVersion;
use aws_sdk_s3::{Client, presigning::PresigningConfig};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{env, path::Path, time::Duration};
use uuid::Uuid;

const MAX_FILE_SIZE: u64 = 1_000_000_000; // 1 GB per file

const ALLOWED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "avif", "mp4", "mov", "m4v", "webm",
];

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
    // Loads variables from .env during local development.
    dotenvy::dotenv().ok();

    // The AWS SDK reads AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY,
    // AWS_REGION and AWS_ENDPOINT_URL from the environment.
    let aws_config = aws_config::defaults(BehaviorVersion::latest()).load().await;

    let state = AppState {
        s3: Client::new(&aws_config),
        bucket: env::var("S3_BUCKET")?,
        event_token: env::var("EVENT_TOKEN")?,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/upload-url", post(create_upload_url))
        .with_state(state);

    // Railway automatically supplies PORT.
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()?;

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;

    println!("Server running on http://localhost:{port}");

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
    check_event_token(&headers, &state.event_token)?;

    if request.size == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "The selected file is empty.".to_string(),
        ));
    }

    if request.size > MAX_FILE_SIZE {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "Files must be smaller than 1 GB.".to_string(),
        ));
    }

    let extension = Path::new(&request.file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "The file must have an extension.".to_string(),
            )
        })?;

    if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("Files ending in .{extension} are not supported."),
        ));
    }

    let valid_content_type = request.content_type.starts_with("image/")
        || request.content_type.starts_with("video/")
        || request.content_type == "application/octet-stream";

    if !valid_content_type {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "Only photos and videos can be uploaded.".to_string(),
        ));
    }

    // We generate the stored filename ourselves to avoid collisions and to prevent users from overwriting each other's files.
    let object_key = format!(
        "wedding/{}/{}.{}",
        "guest-uploads",
        Uuid::new_v4(),
        extension
    );

    let presigning_config =
        PresigningConfig::expires_in(Duration::from_secs(10 * 60)).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not configure upload URL: {error}"),
            )
        })?;

    let presigned_request = state
        .s3
        .put_object()
        .bucket(&state.bucket)
        .key(&object_key)
        .content_type(&request.content_type)
        .presigned(presigning_config)
        .await
        .map_err(|error| {
            eprintln!("Presigning error: {error:?}");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Could not prepare the upload.".to_string(),
            )
        })?;

    Ok(Json(UploadResponse {
        upload_url: presigned_request.uri().to_string(),
        object_key,
    }))
}

fn check_event_token(
    headers: &HeaderMap,
    expected_token: &str,
) -> Result<(), (StatusCode, String)> {
    let supplied_token = headers
        .get("x-event-token")
        .and_then(|value| value.to_str().ok());

    if supplied_token != Some(expected_token) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "This upload link is invalid.".to_string(),
        ));
    }

    Ok(())
}
