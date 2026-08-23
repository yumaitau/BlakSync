use std::collections::BTreeMap;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

use axum::Json;
use axum::Router;
#[cfg(test)]
use axum::body::Body;
use axum::extract::{Path as AxumPath, Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    CACHE_CONTROL, HOST, ORIGIN, VARY,
};
use axum::http::uri::Authority;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::services::{ServeDir, ServeFile};

use crate::config::default_config_dir;
use crate::notes::AccessNotesStore;
use crate::syncthing::{FolderStatus, SyncthingClient};
use crate::{Error, Result};

#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub syncthing_url: String,
    pub api_key: String,
    pub config_dir: PathBuf,
    pub static_root: PathBuf,
}

impl ServerOptions {
    pub fn from_environment(
        host: Option<String>,
        port: Option<u16>,
        static_root: Option<PathBuf>,
    ) -> Result<Self> {
        let api_key = std::env::var("BLAKSYNC_API_KEY").map_err(|_| {
            Error::Config(
                "Set BLAKSYNC_API_KEY to your local Syncthing API key. The key is not printed."
                    .into(),
            )
        })?;
        Ok(Self {
            host: host
                .or_else(|| std::env::var("BLAKSYNC_GUI_HOST").ok())
                .unwrap_or_else(|| "127.0.0.1".into()),
            port: port
                .or_else(|| {
                    std::env::var("BLAKSYNC_GUI_PORT")
                        .ok()
                        .and_then(|value| value.parse().ok())
                })
                .unwrap_or(8385),
            syncthing_url: std::env::var("BLAKSYNC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8384".into()),
            api_key,
            config_dir: default_config_dir(),
            static_root: static_root.unwrap_or_else(|| PathBuf::from("dist")),
        })
    }
}

#[derive(Clone)]
struct AppState {
    client: SyncthingClient,
    notes: AccessNotesStore,
    syncthing_url: String,
}

#[derive(Clone)]
struct BrowserPolicy {
    bind_host: String,
}

pub async fn serve(options: ServerOptions) -> Result<()> {
    let address = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    let local = listener.local_addr()?;
    let app = build_router(&options)?;
    println!("BlakSync GUI listening on http://{local}");
    println!("Syncthing stock GUI remains available as a fallback on its own address.");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

pub fn build_router(options: &ServerOptions) -> Result<Router> {
    let state = AppState {
        client: SyncthingClient::new(Some(&options.syncthing_url), Some(&options.api_key))?,
        notes: AccessNotesStore::new(&options.config_dir)?,
        syncthing_url: options.syncthing_url.clone(),
    };
    let policy = BrowserPolicy {
        bind_host: options.host.clone(),
    };
    let api = Router::new()
        .route("/health", get(health))
        .route("/overview", get(overview))
        .route("/folders", get(folders).post(add_folder))
        .route("/folders/{folder_id}/share", post(share_folder))
        .route("/folders/{folder_id}/unshare", post(unshare_folder))
        .route("/folders/{folder_id}/pause", post(pause_folder))
        .route("/folders/{folder_id}/resume", post(resume_folder))
        .route("/folders/{folder_id}/note", patch(set_note))
        .route("/devices", get(devices).post(add_device))
        .route("/pending/devices/{device_id}/accept", post(accept_device))
        .route("/pending/devices/{device_id}/deny", post(deny_device))
        .route("/this-device", get(this_device))
        .route("/settings", get(settings))
        .fallback(not_found)
        .route_layer(middleware::from_fn_with_state(policy, browser_policy));

    let index = options.static_root.join("index.html");
    let index_service = ServeFile::new(index);
    Ok(Router::new()
        .nest("/api", api)
        .route_service("/", index_service.clone())
        .route_service("/this-device", index_service.clone())
        .route_service("/devices", index_service.clone())
        .route_service("/pending", index_service.clone())
        .route_service("/settings", index_service)
        .fallback_service(ServeDir::new(&options.static_root))
        .with_state(state))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}

async fn overview(State(state): State<AppState>) -> ApiResult {
    let this_device = state.client.this_device().await?;
    let folders = state.client.folder_statuses().await?;
    let remote_devices = state.client.remote_device_statuses().await?;
    let pending_devices = state.client.pending_devices().await?;
    let pending_folders = state.client.pending_folders().await?;
    let notes = state.notes.read_all()?;
    let devices = state.client.devices().await?;
    let device_names: BTreeMap<String, String> = devices
        .iter()
        .map(|device| {
            let id = field_string(device, "deviceID");
            let name = field_string(device, "name");
            (id.clone(), if name.is_empty() { id } else { name })
        })
        .collect();
    let folders: Vec<Value> = folders
        .iter()
        .map(|folder| enrich_folder(folder, &notes, &device_names))
        .collect::<Result<_>>()?;
    let pending_devices = pending_devices
        .as_object()
        .into_iter()
        .flatten()
        .map(|(device_id, info)| {
            json!({
                "deviceId": device_id,
                "name": nonempty_or(field_string(info, "name"), device_id),
                "address": field_string(info, "address"),
                "accessNotes": notes.iter().map(|(folder_id, note)| json!({
                    "folderId": folder_id,
                    "note": note,
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let pending_folders = pending_folders
        .as_object()
        .into_iter()
        .flatten()
        .map(|(folder_id, info)| {
            json!({
                "folderId": folder_id,
                "label": nonempty_or(field_string(info, "label"), folder_id),
                "offeredBy": info.get("offeredBy").cloned().unwrap_or_else(|| json!({})),
                "accessNote": notes.get(folder_id).cloned().unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    ok(json!({
        "thisDevice": this_device,
        "folders": folders,
        "remoteDevices": remote_devices,
        "pending": {
            "devices": pending_devices,
            "folders": pending_folders,
        },
        "syncthingUrl": state.syncthing_url,
    }))
}

async fn folders(State(state): State<AppState>) -> ApiResult {
    let folders = state.client.folder_statuses().await?;
    let notes = state.notes.read_all()?;
    let devices = state.client.devices().await?;
    let names = devices
        .iter()
        .map(|device| {
            let id = field_string(device, "deviceID");
            let name = field_string(device, "name");
            (id.clone(), if name.is_empty() { id } else { name })
        })
        .collect();
    let output = folders
        .iter()
        .map(|folder| enrich_folder(folder, &notes, &names))
        .collect::<Result<Vec<_>>>()?;
    ok(json!(output))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddFolder {
    id: String,
    path: String,
    label: Option<String>,
    access_note: Option<String>,
}

async fn add_folder(State(state): State<AppState>, Json(input): Json<AddFolder>) -> ApiResult {
    required(&input.id, "id")?;
    required(&input.path, "path")?;
    let folder = state
        .client
        .add_folder(&input.id, &input.path, input.label.as_deref())
        .await?;
    if let Some(note) = input.access_note.filter(|note| !note.is_empty()) {
        state.notes.set(&input.id, &note)?;
    }
    created(json!({
        "id": field_string(&folder, "id"),
        "accessNote": state.notes.get(&input.id)?,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceInput {
    device_id: String,
    name: Option<String>,
}

async fn share_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<DeviceInput>,
) -> ApiResult {
    state
        .client
        .share_folder(&folder_id, &input.device_id)
        .await?;
    ok(json!({ "shared": true }))
}

async fn unshare_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<DeviceInput>,
) -> ApiResult {
    state
        .client
        .unshare_folder(&folder_id, &input.device_id)
        .await?;
    ok(json!({ "shared": false }))
}

async fn pause_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
) -> ApiResult {
    state.client.set_folder_paused(&folder_id, true).await?;
    ok(json!({ "paused": true }))
}

async fn resume_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
) -> ApiResult {
    state.client.set_folder_paused(&folder_id, false).await?;
    ok(json!({ "paused": false }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NoteInput {
    access_note: Option<String>,
}

async fn set_note(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<NoteInput>,
) -> ApiResult {
    let access_note = state
        .notes
        .set(&folder_id, input.access_note.as_deref().unwrap_or_default())?;
    ok(json!({ "id": folder_id, "accessNote": access_note }))
}

async fn devices(State(state): State<AppState>) -> ApiResult {
    ok(json!(state.client.remote_device_statuses().await?))
}

async fn add_device(State(state): State<AppState>, Json(input): Json<DeviceInput>) -> ApiResult {
    let device = state
        .client
        .add_device(&input.device_id, input.name.as_deref().unwrap_or_default())
        .await?;
    created(json!({
        "deviceId": field_string(&device, "deviceID"),
        "name": field_string(&device, "name"),
    }))
}

#[derive(Default, Deserialize)]
struct AcceptInput {
    name: Option<String>,
}

async fn accept_device(
    State(state): State<AppState>,
    AxumPath(device_id): AxumPath<String>,
    input: Option<Json<AcceptInput>>,
) -> ApiResult {
    let name = input.and_then(|Json(input)| input.name).unwrap_or_default();
    let device = state.client.add_device(&device_id, &name).await?;
    ok(json!({
        "accepted": true,
        "deviceId": field_string(&device, "deviceID"),
    }))
}

async fn deny_device(
    State(state): State<AppState>,
    AxumPath(device_id): AxumPath<String>,
) -> ApiResult {
    state.client.deny_pending_device(&device_id).await?;
    ok(json!({ "denied": true }))
}

async fn this_device(State(state): State<AppState>) -> ApiResult {
    ok(json!(state.client.this_device().await?))
}

async fn settings(State(state): State<AppState>) -> ApiResult {
    let device = state.client.this_device().await?;
    let stock_gui = format!("http://{}", device.syncthing_gui_address);
    ok(json!({
        "thisDevice": device,
        "syncthingUrl": state.syncthing_url,
        "guiBind": "127.0.0.1",
        "accessNotesPath": state.notes.file_path(),
        "stockGuiFallback": stock_gui,
    }))
}

async fn not_found() -> ApiResult {
    Err(ApiError(Error::NotFound("Not found".into())))
}

fn enrich_folder(
    folder: &FolderStatus,
    notes: &BTreeMap<String, String>,
    names: &BTreeMap<String, String>,
) -> Result<Value> {
    let mut value = serde_json::to_value(folder)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::Config("Could not serialize folder status".into()))?;
    object.insert(
        "accessNote".into(),
        json!(notes.get(&folder.id).cloned().unwrap_or_default()),
    );
    object.insert(
        "sharedWith".into(),
        json!(
            folder
                .devices
                .iter()
                .map(|device_id| json!({
                    "deviceId": device_id,
                    "name": names.get(device_id).unwrap_or(device_id),
                }))
                .collect::<Vec<_>>()
        ),
    );
    Ok(value)
}

async fn browser_policy(
    State(policy): State<BrowserPolicy>,
    request: Request,
    next: Next,
) -> Response {
    let host_header = match request
        .headers()
        .get(HOST)
        .and_then(|value| value.to_str().ok())
    {
        Some(host) => host,
        None => return error_response(StatusCode::BAD_REQUEST, "Invalid Host header"),
    };
    let hostname = Authority::from_str(host_header)
        .map(|authority| authority.host().to_string())
        .unwrap_or_default();
    let host_allowed = if is_loopback(&policy.bind_host) {
        is_loopback(&hostname)
    } else {
        hostname == policy.bind_host
    };
    if !host_allowed {
        return error_response(StatusCode::FORBIDDEN, "Host is not allowed");
    }

    let origin = request
        .headers()
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if let Some(origin) = &origin {
        let same_origin = origin == &format!("http://{host_header}");
        let vite_origin = reqwest::Url::parse(origin).is_ok_and(|url| {
            url.scheme() == "http"
                && url.host_str().is_some_and(is_loopback)
                && url.port() == Some(5173)
        });
        if !same_origin && !vite_origin {
            return error_response(StatusCode::FORBIDDEN, "Origin is not allowed");
        }
    }

    let mut response = if request.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET,POST,PATCH,DELETE,OPTIONS"),
    );
    headers.insert(
        ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type"),
    );
    if let Some(origin) = origin.and_then(|origin| HeaderValue::from_str(&origin).ok()) {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.insert(VARY, HeaderValue::from_static("Origin"));
    }
    response
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

fn required(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        return Err(Error::Config(format!("Missing {field}")));
    }
    Ok(())
}

fn field_string(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn nonempty_or(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

type ApiResult = std::result::Result<(StatusCode, Json<Value>), ApiError>;

fn ok(value: Value) -> ApiResult {
    Ok((StatusCode::OK, Json(value)))
}

fn created(value: Value) -> ApiResult {
    Ok((StatusCode::CREATED, Json(value)))
}

#[derive(Debug)]
struct ApiError(Error);

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::Role(_) => StatusCode::FORBIDDEN,
            Error::Syncthing { status, .. } => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            Error::Config(_) | Error::Json(_) => StatusCode::BAD_REQUEST,
            Error::Http(_) | Error::Io(_) | Error::Process(_) | Error::Csv(_) => {
                StatusCode::BAD_GATEWAY
            }
        };
        error_response(status, &self.0.to_string())
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let mut response = (status, Json(json!({ "error": message }))).into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn shutdown_signal() {
    let control_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = control_c => {},
        () = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    use super::*;

    fn test_options(root: &Path) -> ServerOptions {
        ServerOptions {
            host: "127.0.0.1".into(),
            port: 0,
            syncthing_url: "http://127.0.0.1:1".into(),
            api_key: "test-key".into(),
            config_dir: root.join("config"),
            static_root: root.join("dist"),
        }
    }

    #[tokio::test]
    async fn health_defaults_to_loopback_and_rejects_foreign_origin() {
        let temporary = tempfile::tempdir().unwrap();
        let app = build_router(&test_options(temporary.path())).unwrap();
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .header(HOST, "127.0.0.1:8385")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/folders")
                    .header(HOST, "127.0.0.1:8385")
                    .header(ORIGIN, "https://example.invalid")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"id":"blocked","path":"/tmp/blocked"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method(Method::OPTIONS)
                    .uri("/api/folders")
                    .header(HOST, "127.0.0.1:8385")
                    .header(ORIGIN, "http://127.0.0.1:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&HeaderValue::from_static("http://127.0.0.1:5173"))
        );

        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/api/health")
                    .header(HOST, "attacker.invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
