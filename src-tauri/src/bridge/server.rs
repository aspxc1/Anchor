use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket},
        DefaultBodyLimit, Query, State, WebSocketUpgrade,
    },
    http::{header, HeaderName, HeaderValue, Method, Request as HttpRequest, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    io,
    net::{Ipv4Addr, SocketAddrV4},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::{oneshot, watch};
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone, Serialize)]
pub struct Status {
    pub state: String,
    pub session_id: Option<String>,
    pub goal: String,
    pub remaining_seconds: u64,
    pub hall_passes_remaining: u8,
    pub hall_pass_remaining_seconds: u64,
    pub allowed_domains: Vec<String>,
    pub allow_localhost_domains: bool,
}
#[derive(Deserialize)]
pub struct Violation {
    pub domain: String,
    pub url: String,
    pub tab_title: String,
    pub session_id: Option<String>,
}
pub enum Request {
    Violation(Violation),
    HallPass,
}
pub enum ApiError {
    Conflict,
    Internal,
    Invalid,
}
pub type Reply = Result<Status, ApiError>;
pub type Dispatch = Arc<dyn Fn(Request, oneshot::Sender<Reply>) + Send + Sync>;

#[derive(Clone)]
struct AppState {
    token: watch::Receiver<String>,
    status: watch::Receiver<Status>,
    dispatch: Dispatch,
    shutdown: watch::Receiver<bool>,
    connections: Arc<AtomicUsize>,
    last_request: Arc<Mutex<Option<Instant>>>,
}
pub struct Server {
    shutdown: watch::Sender<bool>,
    token: watch::Sender<String>,
    connections: Arc<AtomicUsize>,
    last_request: Arc<Mutex<Option<Instant>>>,
}
impl Server {
    pub fn connected(&self) -> bool {
        self.connections.load(Ordering::Relaxed) > 0
            || self
                .last_request
                .lock()
                .ok()
                .and_then(|v| *v)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(60))
    }
    pub fn rotate_token(&self, token: String) {
        self.token.send_replace(token);
        if let Ok(mut last) = self.last_request.lock() {
            *last = None;
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
    }
}

pub fn start(
    port: u16,
    token: String,
    status: watch::Receiver<Status>,
    dispatch: Dispatch,
) -> io::Result<Server> {
    let listener = std::net::TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))?;
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let (shutdown, shutdown_rx) = watch::channel(false);
    let (token_tx, token_rx) = watch::channel(token);
    let connections = Arc::new(AtomicUsize::new(0));
    let last_request = Arc::new(Mutex::new(None));
    let state = AppState {
        token: token_rx,
        status,
        dispatch,
        shutdown: shutdown_rx.clone(),
        connections: connections.clone(),
        last_request: last_request.clone(),
    };
    let mut stop = shutdown_rx;
    tokio::spawn(async move {
        let result = axum::serve(listener, router(state))
            .with_graceful_shutdown(async move {
                while !*stop.borrow() {
                    if stop.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await;
        if let Err(error) = result {
            eprintln!("The localhost bridge stopped: {error}");
        }
    });
    Ok(Server {
        shutdown,
        token: token_tx,
        connections,
        last_request,
    })
}
fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/ws", get(ws))
        .route("/api/v1/violation", post(violation))
        .route("/api/v1/hallpass", post(hallpass))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), token_guard))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::predicate(|origin, _| {
                    origin.to_str().is_ok_and(valid_origin)
                }))
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([
                    HeaderName::from_static("x-anchor-token"),
                    header::CONTENT_TYPE,
                ]),
        )
        .layer(middleware::from_fn(origin_guard))
        .with_state(state)
}
fn valid_origin(origin: &str) -> bool {
    let Some(id) = origin
        .strip_prefix("chrome-extension://")
        .or_else(|| origin.strip_prefix("extension://"))
    else {
        return false;
    };
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}
fn matches_token(supplied: &str, expected: &str) -> bool {
    if expected.is_empty() || supplied.len() != expected.len() {
        return false;
    }
    supplied
        .bytes()
        .zip(expected.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}
async fn origin_guard(req: HttpRequest<Body>, next: Next) -> Response {
    if req
        .headers()
        .get(header::ORIGIN)
        .is_some_and(|origin| !origin.to_str().is_ok_and(valid_origin))
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(req).await
}

async fn token_guard(
    State(state): State<AppState>,
    req: HttpRequest<Body>,
    next: Next,
) -> Response {
    let is_ws = req.uri().path() == "/api/v1/ws";
    let token = if is_ws {
        // Query extraction is deferred to the handler; decode percent escapes using Axum's extractor.
        Query::<WsQuery>::try_from_uri(req.uri())
            .ok()
            .map(|q| q.0.token)
    } else {
        req.headers()
            .get("x-anchor-token")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    if !token
        .as_deref()
        .is_some_and(|t| matches_token(t, &state.token.borrow()))
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if let Ok(mut last) = state.last_request.lock() {
        *last = Some(Instant::now());
    }
    let mut response = next.run(req).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn status(State(state): State<AppState>) -> Json<Status> {
    Json(state.status.borrow().clone())
}
async fn violation(State(state): State<AppState>, Json(value): Json<Violation>) -> Response {
    dispatch(state, Request::Violation(value)).await
}
async fn hallpass(State(state): State<AppState>) -> Response {
    dispatch(state, Request::HallPass).await
}
async fn dispatch(state: AppState, request: Request) -> Response {
    let (tx, rx) = oneshot::channel();
    (state.dispatch)(request, tx);
    match tokio::time::timeout(Duration::from_secs(10), rx).await {
        Ok(Ok(Ok(status))) => Json(status).into_response(),
        Ok(Ok(Err(ApiError::Conflict))) => StatusCode::CONFLICT.into_response(),
        Ok(Ok(Err(ApiError::Invalid))) => StatusCode::BAD_REQUEST.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
#[derive(Deserialize)]
struct WsQuery {
    token: String,
}
async fn ws(
    State(mut state): State<AppState>,
    Query(query): Query<WsQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !matches_token(&query.token, &state.token.borrow_and_update()) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    upgrade
        .max_message_size(16 * 1024)
        .max_frame_size(16 * 1024)
        .on_upgrade(move |socket| websocket(socket, state))
}
struct Connection(Arc<AtomicUsize>);
impl Drop for Connection {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}
async fn websocket(mut socket: WebSocket, state: AppState) {
    if *state.shutdown.borrow() {
        return;
    }
    state.connections.fetch_add(1, Ordering::Relaxed);
    let _connection = Connection(state.connections.clone());
    let mut status = state.status.clone();
    let mut shutdown = state.shutdown.clone();
    let mut token = state.token.clone();
    let snapshot = serde_json::to_string(&*status.borrow_and_update()).ok();
    if let Some(snapshot) = snapshot {
        if socket.send(Message::Text(snapshot.into())).await.is_err() {
            return;
        }
    }
    let mut keepalive = tokio::time::interval(Duration::from_secs(20));
    keepalive.tick().await;
    loop {
        tokio::select! {
            changed = shutdown.changed() => { if changed.is_err() || *shutdown.borrow() { break; } }
            _ = token.changed() => break,
            changed = status.changed() => {
                if changed.is_err() { break; }
                let snapshot = serde_json::to_string(&*status.borrow_and_update()).ok();
                if let Some(snapshot) = snapshot { if socket.send(Message::Text(snapshot.into())).await.is_err() { break; } }
            }
            _ = keepalive.tick() => { if socket.send(Message::Ping(Vec::new().into())).await.is_err() { break; } }
            message = socket.recv() => match message { Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break, _ => {} },
        }
    }
    let _ = socket.send(Message::Close(None)).await;
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request as HttpRequest, StatusCode},
    };
    use tower::ServiceExt;

    fn test_router() -> Router {
        let (_, status) = watch::channel(Status {
            state: "Idle".into(),
            session_id: None,
            goal: "".into(),
            remaining_seconds: 0,
            hall_passes_remaining: 0,
            hall_pass_remaining_seconds: 0,
            allowed_domains: vec![],
            allow_localhost_domains: false,
        });
        let (_, token) = watch::channel("secret".into());
        let (_, shutdown) = watch::channel(false);
        router(AppState {
            token,
            status,
            shutdown,
            dispatch: Arc::new(|_, tx| {
                let _ = tx.send(Err(ApiError::Conflict));
            }),
            connections: Arc::new(AtomicUsize::new(0)),
            last_request: Arc::new(Mutex::new(None)),
        })
    }

    #[tokio::test]
    async fn authenticated_status_is_returned() {
        let response = test_router()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/v1/status")
                    .header("x-anchor-token", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_hostile_origin_before_checking_credentials() {
        let app = test_router();
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/v1/status")
                    .header("origin", "https://evil.example")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn status_requires_authentication() {
        let app = test_router();
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/v1/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn preflight_allows_extension_origin_and_headers() {
        let response = test_router()
            .oneshot(
                HttpRequest::builder()
                    .method("OPTIONS")
                    .uri("/api/v1/status")
                    .header("origin", "chrome-extension://abcdefghijklmnop")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(response.status().is_success());
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "chrome-extension://abcdefghijklmnop"
        );
        assert!(response.headers()["access-control-allow-headers"]
            .to_str()
            .unwrap()
            .contains("x-anchor-token"));
    }

    #[tokio::test]
    async fn reports_dispatch_conflict() {
        let response = test_router()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/v1/hallpass")
                    .header("x-anchor-token", "secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn rejects_oversized_violation() {
        let response = test_router()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/v1/violation")
                    .header("x-anchor-token", "secret")
                    .header("content-type", "application/json")
                    .body(Body::from("x".repeat(16 * 1024 + 1)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
    #[tokio::test]
    async fn token_rotation_changes_http_authentication() {
        let (_, status) = watch::channel(Status {
            state: "Idle".into(),
            session_id: None,
            goal: "".into(),
            remaining_seconds: 0,
            hall_passes_remaining: 0,
            hall_pass_remaining_seconds: 0,
            allowed_domains: vec![],
            allow_localhost_domains: false,
        });
        let (token_tx, token) = watch::channel("old".to_string());
        let (_, shutdown) = watch::channel(false);
        let app = router(AppState {
            token,
            status,
            shutdown,
            dispatch: Arc::new(|_, tx| {
                let _ = tx.send(Err(ApiError::Conflict));
            }),
            connections: Arc::new(AtomicUsize::new(0)),
            last_request: Arc::new(Mutex::new(None)),
        });
        token_tx.send_replace("new".into());
        let request = |token: &str| {
            HttpRequest::builder()
                .uri("/api/v1/status")
                .header("x-anchor-token", token)
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(
            app.clone().oneshot(request("old")).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            app.oneshot(request("new")).await.unwrap().status(),
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn browser_violation_persists_and_hall_pass_updates_session_status() {
        use crate::{
            storage::{
                history::{BlockSource, BlockedRecord},
                Storage,
            },
            timing::Timing,
        };
        let root =
            std::env::temp_dir().join(format!("anchor-bridge-test-{}", uuid::Uuid::new_v4()));
        let storage = Storage::open(&root).unwrap();
        let id = storage.start_session("Read docs", 25, "[]").unwrap();
        let state = Status {
            state: "ActiveFocus".into(),
            session_id: Some(id.clone()),
            goal: "Read docs".into(),
            remaining_seconds: 1500,
            hall_passes_remaining: 1,
            hall_pass_remaining_seconds: 0,
            allowed_domains: vec!["docs.rs".into()],
            allow_localhost_domains: true,
        };
        let (publisher, status) = watch::channel(state.clone());
        let (_, token) = watch::channel("secret".into());
        let (_, shutdown) = watch::channel(false);
        let state_id = id.clone();
        let storage = Arc::new(Mutex::new(storage));
        let db = storage.clone();
        let timing = Mutex::new(Timing::new(25, 5, 1, Instant::now()));
        let app = router(AppState {
            token,
            status,
            shutdown,
            connections: Arc::new(AtomicUsize::new(0)),
            last_request: Arc::new(Mutex::new(None)),
            dispatch: Arc::new(move |request, tx| {
                let mut updated = publisher.borrow().clone();
                let result = match request {
                    Request::Violation(violation) => {
                        assert_eq!(violation.url, "https://reddit.com/");
                        db.lock()
                            .unwrap()
                            .record_switches(&[BlockedRecord {
                                source: BlockSource::Browser,
                                session_id: state_id.clone(),
                                occurred_at: chrono::Local::now().to_rfc3339(),
                                process_name: violation.domain,
                                window_title: violation.tab_title,
                            }])
                            .map_err(|_| ApiError::Internal)
                    }
                    Request::HallPass => {
                        let mut clock = timing.lock().unwrap();
                        if clock.use_pass(Instant::now()) {
                            updated.state = clock.phase.name().into();
                            updated.hall_passes_remaining = clock.passes_remaining;
                            updated.hall_pass_remaining_seconds =
                                clock.seconds_left(Instant::now());
                            publisher.send_replace(updated.clone());
                            Ok(())
                        } else {
                            Err(ApiError::Conflict)
                        }
                    }
                };
                let _ = tx.send(result.map(|_| updated));
            }),
        });
        let post = |path: &str, body: &'static str| {
            HttpRequest::builder()
                .method("POST")
                .uri(path)
                .header("x-anchor-token", "secret")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap()
        };
        let response = app
            .clone()
            .oneshot(post(
                "/api/v1/violation",
                r#"{"domain":"reddit.com","url":"https://reddit.com/","tab_title":"Reddit"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .clone()
            .oneshot(post("/api/v1/hallpass", ""))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 16_384)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["state"], "HallPass");
        assert_eq!(value["hall_passes_remaining"], 0);
        assert_eq!(
            app.oneshot(post("/api/v1/hallpass", ""))
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );

        let stored = rusqlite::Connection::open(root.join("anchor.db")).unwrap();
        let (source, name, title): (String, String, String) = stored.query_row(
            "SELECT source, process_name, window_title FROM blocked_switches WHERE session_id = ?1", [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(
            (source.as_str(), name.as_str(), title.as_str()),
            ("browser", "reddit.com", "Reddit")
        );
        drop(stored);
        drop(storage);
        std::fs::remove_dir_all(root).unwrap();
    }
}
