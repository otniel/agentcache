use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::normalizer::normalize_request;

pub async fn handle_messages(
    Json(mut body): Json<Value>,
) -> impl IntoResponse {
    let request_id = uuid::Uuid::new_v4().to_string();
    info!(request_id = %request_id, "Incoming request");

    // Step 1: Normalize the request
    let before = serde_json::to_string(&body).unwrap_or_default();
    normalize_request(&mut body);
    let after = serde_json::to_string(&body).unwrap_or_default();

    if before != after {
        info!(
            request_id = %request_id,
            "Request normalized — tool definitions or content modified for cache stability"
        );
    } else {
        info!(request_id = %request_id, "Request already deterministic — no normalization needed");
    }

    // Step 2: Forward to Anthropic
    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) => k,
        Err(_) => {
            warn!("ANTHROPIC_API_KEY not set");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "ANTHROPIC_API_KEY not configured"})),
            );
        }
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            let json: Value = resp.json().await.unwrap_or(Value::Null);
            info!(request_id = %request_id, status = %status, "Upstream response received");
            (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK), Json(json))
        }
        Err(e) => {
            warn!(request_id = %request_id, error = %e, "Upstream request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"error": "Upstream request failed", "detail": e.to_string()})),
            )
        }
    }
}
