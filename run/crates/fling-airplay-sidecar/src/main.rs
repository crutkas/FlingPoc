use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use constant_time_eq::constant_time_eq;
use fling_airplay::{AirPlayEngine, AirPlayError, Capabilities, PairingSession, PlaybackStatus};
use serde::{Deserialize, Serialize};

const MAX_REQUEST_BYTES: usize = 1_048_576;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
struct AppState {
    engine: Arc<AirPlayEngine>,
    authorization: Arc<[u8]>,
}

impl AppState {
    fn new(engine: AirPlayEngine, token: &str) -> Self {
        Self {
            engine: Arc::new(engine),
            authorization: Arc::from(bearer_authorization(token).into_bytes()),
        }
    }
}

fn bearer_authorization(token: &str) -> String {
    ["Bearer", token].join(" ")
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            format!("Invalid request: {}", message.into()),
        )
    }
}

impl From<AirPlayError> for ApiError {
    fn from(error: AirPlayError) -> Self {
        let status = match error {
            AirPlayError::Unsupported(_) => StatusCode::NOT_IMPLEMENTED,
            AirPlayError::Discovery(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceRequest {
    device_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairFinishRequest {
    session_id: String,
    pin: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayRequest {
    device_id: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileRequest {
    device_id: String,
    path: String,
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/devices", get(devices))
        .route("/v1/pair/start", post(pair_start))
        .route("/v1/pair/finish", post(pair_finish))
        .route("/v1/play", post(play))
        .route("/v1/file", post(play_file))
        .route("/v1/mirror", post(mirror))
        .route("/v1/status", post(status))
        .route("/v1/stop", post(stop))
        .fallback(route_not_found)
        .method_not_allowed_fallback(route_not_found)
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(middleware::from_fn(request_timeout))
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .with_state(state)
}

async fn authorize(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let authorized = authorization_bytes(request.headers())
        .is_some_and(|provided| constant_time_eq(provided, &state.authorization));
    if !authorized {
        return ApiError::new(StatusCode::UNAUTHORIZED, "Unauthorized.").into_response();
    }
    next.run(request).await
}

fn authorization_bytes(headers: &HeaderMap) -> Option<&[u8]> {
    headers
        .get(AUTHORIZATION)
        .map(axum::http::HeaderValue::as_bytes)
}

async fn request_timeout(request: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::new(StatusCode::REQUEST_TIMEOUT, "Request timed out.").into_response(),
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn capabilities(State(state): State<AppState>) -> Json<Capabilities> {
    Json(state.engine.capabilities())
}

async fn devices(
    State(state): State<AppState>,
) -> Result<Json<Vec<fling_airplay::Device>>, ApiError> {
    Ok(Json(state.engine.devices().await?))
}

async fn pair_start(
    State(state): State<AppState>,
    payload: Result<Json<DeviceRequest>, JsonRejection>,
) -> Result<Json<PairingSession>, ApiError> {
    let request = parse_json(payload)?;
    require_value("deviceId", &request.device_id)?;
    Ok(Json(state.engine.start_pairing(&request.device_id)?))
}

async fn pair_finish(
    State(state): State<AppState>,
    payload: Result<Json<PairFinishRequest>, JsonRejection>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let request = parse_json(payload)?;
    require_value("sessionId", &request.session_id)?;
    state
        .engine
        .finish_pairing(&request.session_id, &request.pin)?;
    Ok(Json(EmptyResponse {}))
}

async fn play(
    State(state): State<AppState>,
    payload: Result<Json<PlayRequest>, JsonRejection>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let request = parse_json(payload)?;
    require_value("deviceId", &request.device_id)?;
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Apple TV playback requires an HTTP or HTTPS URL.",
        ));
    }
    state.engine.play_url(&request.device_id, &request.url)?;
    Ok(Json(EmptyResponse {}))
}

async fn play_file(
    State(state): State<AppState>,
    payload: Result<Json<FileRequest>, JsonRejection>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let request = parse_json(payload)?;
    require_value("deviceId", &request.device_id)?;
    require_value("path", &request.path)?;
    state.engine.play_file(&request.device_id, &request.path)?;
    Ok(Json(EmptyResponse {}))
}

async fn mirror(
    State(state): State<AppState>,
    payload: Result<Json<DeviceRequest>, JsonRejection>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let request = parse_json(payload)?;
    require_value("deviceId", &request.device_id)?;
    state.engine.mirror_hls(&request.device_id)?;
    Ok(Json(EmptyResponse {}))
}

async fn status(
    State(state): State<AppState>,
    payload: Result<Json<DeviceRequest>, JsonRejection>,
) -> Result<Json<PlaybackStatus>, ApiError> {
    let request = parse_json(payload)?;
    require_value("deviceId", &request.device_id)?;
    Ok(Json(state.engine.status(&request.device_id)?))
}

async fn stop(
    State(state): State<AppState>,
    payload: Result<Json<DeviceRequest>, JsonRejection>,
) -> Result<Json<EmptyResponse>, ApiError> {
    let request = parse_json(payload)?;
    require_value("deviceId", &request.device_id)?;
    state.engine.stop(&request.device_id)?;
    Ok(Json(EmptyResponse {}))
}

async fn route_not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "Route not found.")
}

#[derive(Serialize)]
struct EmptyResponse {}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload.map(|Json(value)| value).map_err(|error| {
        if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "Request body is too large.")
        } else {
            ApiError::invalid(error.body_text())
        }
    })
}

fn require_value(name: &str, value: &str) -> Result<(), ApiError> {
    if value.is_empty() {
        return Err(ApiError::invalid(format!("{name} must not be empty")));
    }
    Ok(())
}

fn parse_port(mut args: impl Iterator<Item = String>) -> Result<u16, String> {
    if args.next().as_deref() != Some("--port") {
        return Err("Usage: fling-airplay-sidecar --port <port>".to_string());
    }
    let port = args
        .next()
        .ok_or_else(|| "A port must follow --port.".to_string())?
        .parse()
        .map_err(|_| "The port must be an integer from 0 through 65535.".to_string())?;
    if args.next().is_some() {
        return Err("Unexpected command-line argument.".to_string());
    }
    Ok(port)
}

async fn run() -> Result<(), String> {
    let port = parse_port(std::env::args().skip(1))?;
    let token = std::env::var("FLING_TOKEN")
        .map_err(|_| "FLING_TOKEN must be set.".to_string())
        .and_then(|token| {
            if token.is_empty() {
                Err("FLING_TOKEN must be set.".to_string())
            } else {
                Ok(token)
            }
        })?;
    let state = AppState::new(AirPlayEngine::default(), &token);
    let listener =
        tokio::net::TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
            .await
            .map_err(|error| format!("Could not bind the Rust bridge: {error}"))?;

    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| format!("Rust bridge failed: {error}"))
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use fling_airplay::CapabilityStatus;
    use tower::ServiceExt;

    use super::*;

    const TOKEN: &str = "test-token";

    fn test_app() -> Router {
        app(AppState::new(AirPlayEngine::default(), TOKEN))
    }

    fn request(method: Method, path: &str, body: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        builder = builder.header(AUTHORIZATION, bearer_authorization(TOKEN));
        if body.is_some() {
            builder = builder.header("content-type", "application/json");
        }
        builder
            .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
            .expect("test request is valid")
    }

    async fn json_body<T: serde::de::DeserializeOwned>(response: Response) -> T {
        let bytes = to_bytes(response.into_body(), MAX_REQUEST_BYTES)
            .await
            .expect("response body can be read");
        serde_json::from_slice(&bytes).expect("response body is JSON")
    }

    #[tokio::test]
    async fn every_route_requires_the_bearer_token() {
        let response = test_app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("test request is valid"),
            )
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: serde_json::Value = json_body(response).await;
        assert_eq!(error["error"], "Unauthorized.");
    }

    #[tokio::test]
    async fn health_and_capabilities_are_machine_readable() {
        let health_response = test_app()
            .oneshot(request(Method::GET, "/health", None))
            .await
            .expect("router responds");
        assert_eq!(health_response.status(), StatusCode::OK);
        let health: serde_json::Value = json_body(health_response).await;
        assert_eq!(health["status"], "ok");

        let capability_response = test_app()
            .oneshot(request(Method::GET, "/v1/capabilities", None))
            .await
            .expect("router responds");
        assert_eq!(capability_response.status(), StatusCode::OK);
        let capabilities: Capabilities = json_body(capability_response).await;
        assert_eq!(
            capabilities.native_mirroring.status,
            CapabilityStatus::Unsupported
        );
        assert_eq!(
            capabilities.hls_mirroring.status,
            CapabilityStatus::PythonFallback
        );
    }

    #[tokio::test]
    async fn unported_routes_return_json_not_implemented_errors() {
        let response = test_app()
            .oneshot(request(
                Method::POST,
                "/v1/play",
                Some(r#"{"deviceId":"abc","url":"https://example.test/video.mp4"}"#),
            ))
            .await
            .expect("router responds");

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let error: serde_json::Value = json_body(response).await;
        assert!(
            error["error"]
                .as_str()
                .expect("error is text")
                .contains("Python bridge")
        );
    }

    #[tokio::test]
    async fn contract_validation_and_unknown_routes_use_json_errors() {
        let invalid = test_app()
            .clone()
            .oneshot(request(
                Method::POST,
                "/v1/play",
                Some(r#"{"deviceId":"abc","url":"file:///tmp/video.mp4"}"#),
            ))
            .await
            .expect("router responds");
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

        let missing = test_app()
            .oneshot(request(Method::GET, "/missing", None))
            .await
            .expect("router responds");
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let error: serde_json::Value = json_body(missing).await;
        assert_eq!(error["error"], "Route not found.");
    }

    #[test]
    fn command_line_requires_exactly_one_port() {
        assert_eq!(
            parse_port(["--port".to_string(), "1234".to_string()].into_iter()),
            Ok(1234)
        );
        assert!(parse_port(std::iter::empty()).is_err());
        assert!(parse_port(["--port", "1234", "extra"].map(str::to_string).into_iter()).is_err());
    }
}
