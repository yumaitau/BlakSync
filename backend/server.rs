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
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, HOST, ORIGIN, VARY,
};
use axum::http::uri::Authority;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use serde::Deserialize;
use serde_json::{Value, json};
use tower_http::services::{ServeDir, ServeFile};

use crate::app_settings::{AppSettingsStore, DiscoveryPreset};
use crate::config::resolve_api_key;
use crate::notes::AccessNotesStore;
use crate::org::{self, OrgOverlay, Role};
use crate::syncthing::{FolderStatus, SyncthingClient, device_qr_svg};
use crate::tls;
use crate::updates;
use crate::version::{PINNED_SYNCTHING, blaksync_version};
use crate::{Error, Result};

const PRIVACY_NOTICE: &str = "BlakSync does not store your file bytes. Copies live on the devices your organisation already owns. Yuma does not run a cloud drive for this product.\n\nGlobal discovery and volunteer relays can see device IDs and roughly where a device is reachable. That metadata is not a secret from those services. Turn them off under Settings if you only need the local network or Tailscale.\n\nThere is no social login and no government ID. Unshare and revoke stop future copies. Files already on a disk stay there. We cannot wipe a lost laptop.\n\nFor help, email hello@yumait.com.au or the contact on this organisation's profile.";

#[derive(Clone, Debug)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub syncthing_url: String,
    pub api_key: String,
    pub config_dir: PathBuf,
    pub static_root: PathBuf,
    pub tls: bool,
    pub actor: Option<String>,
}

impl ServerOptions {
    pub fn from_environment(
        host: Option<String>,
        port: Option<u16>,
        static_root: Option<PathBuf>,
        config_dir: PathBuf,
        tls: bool,
    ) -> Result<Self> {
        let host = host
            .or_else(|| std::env::var("BLAKSYNC_GUI_HOST").ok())
            .unwrap_or_else(|| "127.0.0.1".into());
        require_loopback_bind(&host)?;
        let env_tls = std::env::var("BLAKSYNC_GUI_TLS")
            .ok()
            .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes"));
        Ok(Self {
            host,
            port: port
                .or_else(|| {
                    std::env::var("BLAKSYNC_GUI_PORT")
                        .ok()
                        .and_then(|value| value.parse().ok())
                })
                .unwrap_or(8385),
            syncthing_url: std::env::var("BLAKSYNC_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8384".into()),
            api_key: resolve_api_key(&config_dir)?,
            config_dir,
            static_root: static_root.unwrap_or_else(|| PathBuf::from("dist")),
            tls: tls || env_tls,
            actor: std::env::var("BLAKSYNC_ACTOR")
                .ok()
                .filter(|value| !value.is_empty()),
        })
    }
}

fn require_loopback_bind(host: &str) -> Result<()> {
    if is_loopback(host) {
        return Ok(());
    }
    Err(Error::Config(
        "The BlakSync GUI must bind to 127.0.0.1, localhost, or ::1. Remote access is out of scope."
            .into(),
    ))
}

#[derive(Clone)]
struct AppState {
    client: SyncthingClient,
    notes: AccessNotesStore,
    org: OrgOverlay,
    settings: AppSettingsStore,
    syncthing_url: String,
    tls: bool,
    actor: Option<String>,
}

impl AppState {
    fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }
}

#[derive(Clone)]
struct BrowserPolicy {
    bind_host: String,
    tls: bool,
}

pub async fn serve(options: ServerOptions) -> Result<()> {
    require_loopback_bind(&options.host)?;
    let address = format!("{}:{}", options.host, options.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    let local = listener.local_addr()?;
    let app = build_router(&options)?;
    let scheme = if options.tls { "https" } else { "http" };
    println!("BlakSync GUI listening on {scheme}://{local}");
    if options.tls {
        println!(
            "HTTP on this port is refused. The local certificate is https-cert.pem in the config directory."
        );
    }
    println!("Syncthing stock GUI remains available as a fallback on its own address.");
    if options.tls {
        let std_listener = listener.into_std()?;
        tls::serve_tls(std_listener, app, &options.config_dir, shutdown_signal()).await?;
    } else {
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;
    }
    Ok(())
}

pub fn build_router(options: &ServerOptions) -> Result<Router> {
    let state = AppState {
        client: SyncthingClient::new(Some(&options.syncthing_url), Some(&options.api_key))?,
        notes: AccessNotesStore::new(&options.config_dir)?,
        org: OrgOverlay::new(&options.config_dir)?,
        settings: AppSettingsStore::new(&options.config_dir)?,
        syncthing_url: options.syncthing_url.clone(),
        tls: options.tls,
        actor: options.actor.clone(),
    };
    let policy = BrowserPolicy {
        bind_host: options.host.clone(),
        tls: options.tls,
    };
    let api = Router::new()
        .route("/health", get(health))
        .route("/overview", get(overview))
        .route("/setup", get(setup_status).post(complete_setup))
        .route("/folders", get(folders).post(add_folder))
        .route("/folders/{folder_id}/share", post(share_folder))
        .route("/folders/{folder_id}/unshare", post(unshare_folder))
        .route("/folders/{folder_id}/pause", post(pause_folder))
        .route("/folders/{folder_id}/resume", post(resume_folder))
        .route("/folders/{folder_id}/note", patch(set_note))
        .route("/folders/{folder_id}/type", patch(set_folder_type))
        .route(
            "/folders/{folder_id}/ignores",
            get(get_ignores).post(set_ignores),
        )
        .route(
            "/folders/{folder_id}/remove-local",
            post(remove_local_folder),
        )
        .route("/folders/pause-all", post(pause_all))
        .route("/folders/resume-all", post(resume_all))
        .route("/devices", get(devices).post(add_device))
        .route("/devices/{device_id}/revoke", post(revoke_device))
        .route("/pending/devices/{device_id}/accept", post(accept_device))
        .route("/pending/devices/{device_id}/deny", post(deny_device))
        .route("/pending/folders/{folder_id}/accept", post(accept_folder))
        .route("/this-device", get(this_device))
        .route("/settings", get(settings).patch(update_settings))
        .route("/settings/discovery", post(set_discovery))
        .route("/settings/bandwidth", post(set_bandwidth))
        .route("/settings/autostart", post(set_autostart))
        .route("/org", get(org_profile))
        .route("/org/members", get(list_members).post(add_member))
        .route("/org/members/{member_id}", patch(set_member_role))
        .route("/audit", get(audit_rows))
        .route("/audit.csv", get(audit_csv))
        .route("/office-health", get(office_health))
        .route("/updates", get(version_check))
        .route("/privacy", get(privacy))
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
        .route_service("/settings", index_service.clone())
        .route_service("/health", index_service.clone())
        .route_service("/audit", index_service.clone())
        .route_service("/privacy", index_service)
        .fallback_service(ServeDir::new(&options.static_root))
        .with_state(state))
}

async fn health() -> Json<Value> {
    Json(json!({ "ok": true, "version": blaksync_version(), "syncthingPin": PINNED_SYNCTHING }))
}

async fn overview(State(state): State<AppState>) -> ApiResult {
    let this_device = state.client.this_device().await?;
    let folders = state.client.folder_statuses().await?;
    let remote_devices = state.client.remote_device_statuses().await?;
    let pending_devices = state.client.pending_devices().await?;
    let pending_folders = state.client.pending_folders().await?;
    let notes = state.notes.read_all()?;
    let devices = state.client.devices().await?;
    let settings = state.settings.load()?;
    let capabilities = state.org.capabilities(state.actor())?;
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
                "shortCode": org::short_device_code(device_id),
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
    let global_paused = !folders.is_empty()
        && folders
            .iter()
            .all(|folder| folder.get("status").and_then(Value::as_str) == Some("Paused"));
    ok(json!({
        "thisDevice": this_device,
        "folders": folders,
        "remoteDevices": remote_devices,
        "pending": {
            "devices": pending_devices,
            "folders": pending_folders,
        },
        "syncthingUrl": state.syncthing_url,
        "capabilities": capabilities,
        "setupNeeded": !state.org.has_org()?,
        "discoveryPreset": settings.discovery_preset.as_str(),
        "globalPaused": global_paused,
        "versionCheck": settings.version_check,
        "tls": state.tls,
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
    folder_type: Option<String>,
}

async fn add_folder(State(state): State<AppState>, Json(input): Json<AddFolder>) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.write_folder,
        "This role cannot add a folder.",
    )?;
    required(&input.id, "id")?;
    required(&input.path, "path")?;
    let folder = state
        .client
        .add_folder_typed(
            &input.id,
            &input.path,
            input.label.as_deref(),
            input.folder_type.as_deref().unwrap_or("sendreceive"),
        )
        .await?;
    if let Some(note) = input.access_note.filter(|note| !note.is_empty()) {
        state.notes.set(&input.id, &note)?;
        if state.org.has_org()? {
            let _ = state.org.add_folder(
                state.actor(),
                &input.id,
                input.label.as_deref().unwrap_or(&input.id),
                &note,
                None,
                Some(&input.path),
            );
        }
    }
    created(json!({
        "id": field_string(&folder, "id"),
        "accessNote": state.notes.get(&input.id)?,
        "folderType": field_string(&folder, "type"),
        "unshareLeavesFiles": true,
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
    state.org.guard(
        state.actor(),
        |caps| caps.share,
        "This role cannot share a folder.",
    )?;
    let device_id = state.client.resolve_device_id(&input.device_id).await?;
    state.client.share_folder(&folder_id, &device_id).await?;
    let _ = state.org.register_device(
        state.actor(),
        &device_id,
        input.name.as_deref().unwrap_or_default(),
    );
    if state.org.has_org()? {
        let _ = state
            .org
            .share_folder(state.actor(), &folder_id, &device_id);
    }
    ok(json!({
        "shared": true,
        "notice": "Sharing starts future copies. Files already on a device stay if you later unshare.",
    }))
}

async fn unshare_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<DeviceInput>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.share,
        "This role cannot unshare a folder.",
    )?;
    let device_id = state.client.resolve_device_id(&input.device_id).await?;
    state.client.unshare_folder(&folder_id, &device_id).await?;
    if state.org.has_org()? {
        let _ = state
            .org
            .unshare_folder(state.actor(), &folder_id, &device_id);
    }
    ok(json!({
        "shared": false,
        "notice": "Unshare stops future copies. Files already received stay on both disks. This does not wipe or erase files remotely.",
    }))
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

async fn pause_all(State(state): State<AppState>) -> ApiResult {
    state.client.pause_all_folders(true).await?;
    ok(json!({ "paused": true, "status": "Paused" }))
}

async fn resume_all(State(state): State<AppState>) -> ApiResult {
    state.client.pause_all_folders(false).await?;
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
    state.org.guard(
        state.actor(),
        |caps| caps.write_folder,
        "This role cannot change access notes.",
    )?;
    let access_note = state
        .notes
        .set(&folder_id, input.access_note.as_deref().unwrap_or_default())?;
    ok(json!({ "id": folder_id, "accessNote": access_note }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderTypeInput {
    folder_type: String,
}

async fn set_folder_type(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<FolderTypeInput>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.write_folder,
        "Changing folder type is an owner or admin action.",
    )?;
    state
        .client
        .set_folder_type(&folder_id, &input.folder_type)
        .await?;
    ok(json!({ "id": folder_id, "folderType": input.folder_type }))
}

async fn get_ignores(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
) -> ApiResult {
    ok(json!({ "ignore": state.client.folder_ignores(&folder_id).await? }))
}

#[derive(Deserialize)]
struct IgnoresInput {
    ignore: Vec<String>,
}

async fn set_ignores(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<IgnoresInput>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.write_folder,
        "This role cannot change ignore patterns.",
    )?;
    let ignore = state
        .client
        .set_folder_ignores(&folder_id, &input.ignore)
        .await?;
    ok(json!({ "ignore": ignore }))
}

#[derive(Deserialize)]
struct RemoveLocalInput {
    confirm: String,
}

async fn remove_local_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<RemoveLocalInput>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.write_folder,
        "This role cannot remove a local folder.",
    )?;
    if input.confirm.trim() != folder_id {
        return Err(ApiError(Error::Config(
            "Type the folder ID to remove it from this device only.".into(),
        )));
    }
    let folders = state.client.folder_statuses().await?;
    let folder = folders
        .iter()
        .find(|folder| folder.id == folder_id)
        .ok_or_else(|| Error::NotFound(format!("Unknown folder: {folder_id}")))?;
    let path = PathBuf::from(&folder.path);
    state.client.remove_folder(&folder_id).await?;
    if path.is_dir() {
        std::fs::remove_dir_all(&path).map_err(Error::from)?;
    }
    ok(json!({
        "removed": true,
        "notice": "The folder was removed on this device only. Other devices keep their copies. This is not a remote wipe.",
    }))
}

async fn devices(State(state): State<AppState>) -> ApiResult {
    ok(json!(state.client.remote_device_statuses().await?))
}

async fn add_device(State(state): State<AppState>, Json(input): Json<DeviceInput>) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.accept,
        "This role cannot add a device.",
    )?;
    let device_id = state.client.resolve_device_id(&input.device_id).await?;
    let device = state
        .client
        .add_device(&device_id, input.name.as_deref().unwrap_or_default())
        .await?;
    let _ = state.org.register_device(
        state.actor(),
        &device_id,
        input.name.as_deref().unwrap_or_default(),
    );
    created(json!({
        "deviceId": field_string(&device, "deviceID"),
        "name": field_string(&device, "name"),
        "shortCode": org::short_device_code(&device_id),
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
    state.org.guard(
        state.actor(),
        |caps| caps.accept,
        "Members cannot accept a new device.",
    )?;
    let device_id = state.client.resolve_device_id(&device_id).await?;
    let name = input.and_then(|Json(input)| input.name).unwrap_or_default();
    let device = state.client.add_device(&device_id, &name).await?;
    if state.org.has_org()? {
        if state.org.pending_prompt(state.actor(), &device_id).is_ok() {
            state.org.accept_device(state.actor(), &device_id)?;
        } else {
            let _ = state.org.register_device(state.actor(), &device_id, &name);
        }
    }
    ok(json!({
        "accepted": true,
        "deviceId": field_string(&device, "deviceID"),
        "notice": "Accepting lets the device connect. Share folders separately. Files already on a disk are not wiped if you later revoke.",
    }))
}

async fn deny_device(
    State(state): State<AppState>,
    AxumPath(device_id): AxumPath<String>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.accept,
        "This role cannot deny a pending device.",
    )?;
    let device_id = state.client.resolve_device_id(&device_id).await?;
    state.client.deny_pending_device(&device_id).await?;
    if state.org.has_org()? {
        let _ = state.org.deny_device(state.actor(), &device_id);
    }
    ok(json!({ "denied": true }))
}

async fn revoke_device(
    State(state): State<AppState>,
    AxumPath(device_id): AxumPath<String>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.revoke,
        "Members cannot revoke a device.",
    )?;
    let device_id = state.client.resolve_device_id(&device_id).await?;
    state.client.revoke_device(&device_id).await?;
    let unshared = if state.org.has_org()? {
        state.org.revoke_device(state.actor(), &device_id)?
    } else {
        Vec::new()
    };
    ok(json!({
        "revoked": true,
        "deviceId": device_id,
        "unsharedFolders": unshared,
        "notice": "The device is removed from every folder on this machine. Files already on the lost disk stay there. This is not a remote wipe.",
        "rotateSteps": [
            "If this device's key.pem leaked, stop BlakSync, delete cert.pem and key.pem in the config directory, then start again to get a new device ID.",
            "On every other device, revoke the old ID, add the new ID, and share folders again.",
            "If only the GUI API key leaked, replace the <apikey> value in config.xml and restart. Do not put the new key on a command line.",
            "Full-disk encryption stays the organisation's job. BlakSync does not encrypt files at rest.",
        ],
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcceptFolderInput {
    path: String,
    device_id: String,
    label: Option<String>,
}

async fn accept_folder(
    State(state): State<AppState>,
    AxumPath(folder_id): AxumPath<String>,
    Json(input): Json<AcceptFolderInput>,
) -> ApiResult {
    required(&input.path, "path")?;
    let device_id = state.client.resolve_device_id(&input.device_id).await?;
    state
        .client
        .add_folder(&folder_id, &input.path, input.label.as_deref())
        .await?;
    state.client.share_folder(&folder_id, &device_id).await?;
    ok(json!({ "accepted": true, "id": folder_id }))
}

async fn this_device(State(state): State<AppState>) -> ApiResult {
    let device = state.client.this_device().await?;
    let settings = state.settings.load()?;
    let options = state.client.options().await.unwrap_or(Value::Null);
    ok(json!({
        "deviceId": device.device_id,
        "name": device.name,
        "version": device.version,
        "syncthingGuiAddress": device.syncthing_gui_address,
        "uptimeSeconds": device.uptime_seconds,
        "inboundBytes": device.inbound_bytes,
        "outboundBytes": device.outbound_bytes,
        "shortCode": org::short_device_code(&device.device_id),
        "qrSvg": device_qr_svg(&device.device_id)?,
        "discoveryPreset": settings.discovery_preset.as_str(),
        "discoveryLabel": settings.discovery_preset.label(),
        "sendLimitKib": options.get("maxSendKbps").and_then(Value::as_i64).unwrap_or(0),
        "receiveLimitKib": options.get("maxRecvKbps").and_then(Value::as_i64).unwrap_or(0),
        "syncthingPin": PINNED_SYNCTHING,
        "blaksyncVersion": blaksync_version(),
    }))
}

async fn settings(State(state): State<AppState>) -> ApiResult {
    let device = state.client.this_device().await?;
    let settings = state.settings.load()?;
    let options = state.client.options().await.unwrap_or(Value::Null);
    let stock_gui = format!("http://{}", device.syncthing_gui_address);
    let capabilities = state.org.capabilities(state.actor())?;
    let org = state.org.get_org().ok();
    ok(json!({
        "thisDevice": device,
        "syncthingUrl": state.syncthing_url,
        "guiBind": "127.0.0.1",
        "accessNotesPath": state.notes.file_path(),
        "stockGuiFallback": stock_gui,
        "tls": state.tls,
        "versionCheck": settings.version_check,
        "startAtLogin": crate::autostart::status().unwrap_or(settings.start_at_login),
        "discoveryPreset": settings.discovery_preset.as_str(),
        "discoveryLabel": settings.discovery_preset.label(),
        "tailscaleListen": settings.tailscale_listen,
        "sendLimitKib": options.get("maxSendKbps").and_then(Value::as_i64).unwrap_or(0),
        "receiveLimitKib": options.get("maxRecvKbps").and_then(Value::as_i64).unwrap_or(0),
        "syncthingPin": PINNED_SYNCTHING,
        "blaksyncVersion": blaksync_version(),
        "capabilities": capabilities,
        "organisation": org,
        "roleRule": "Only an owner can assign roles. An admin cannot create a new owner. The last owner cannot demote themselves.",
        "privacyPath": "/privacy",
        "support": "hello@yumait.com.au",
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsPatch {
    version_check: Option<bool>,
    tls: Option<bool>,
}

async fn update_settings(
    State(state): State<AppState>,
    Json(input): Json<SettingsPatch>,
) -> ApiResult {
    let settings = state.settings.update(|settings| {
        if let Some(value) = input.version_check {
            settings.version_check = value;
        }
        if let Some(value) = input.tls {
            settings.tls = value;
        }
    })?;
    ok(json!({
        "versionCheck": settings.version_check,
        "tls": settings.tls,
        "notice": if input.tls == Some(true) {
            "Restart `blaksync gui --tls` to serve HTTPS. HTTP on the same port is then refused."
        } else {
            ""
        },
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryInput {
    preset: String,
    tailscale_listen: Option<String>,
}

async fn set_discovery(
    State(state): State<AppState>,
    Json(input): Json<DiscoveryInput>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.edit_settings,
        "This role cannot change discovery.",
    )?;
    let preset = DiscoveryPreset::parse(&input.preset)?;
    state
        .client
        .apply_discovery(preset, input.tailscale_listen.as_deref())
        .await?;
    state.settings.update(|settings| {
        settings.discovery_preset = preset;
        if let Some(listen) = &input.tailscale_listen {
            settings.tailscale_listen = listen.clone();
        }
    })?;
    ok(json!({
        "preset": preset.as_str(),
        "label": preset.label(),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BandwidthInput {
    send_limit_kib: i64,
    receive_limit_kib: i64,
}

async fn set_bandwidth(
    State(state): State<AppState>,
    Json(input): Json<BandwidthInput>,
) -> ApiResult {
    state
        .client
        .apply_bandwidth(input.send_limit_kib, input.receive_limit_kib)
        .await?;
    ok(json!({
        "sendLimitKib": input.send_limit_kib,
        "receiveLimitKib": input.receive_limit_kib,
        "notice": "Limits persist in Syncthing's config and apply after the next transfer window.",
    }))
}

#[derive(Deserialize)]
struct AutostartInput {
    enabled: bool,
}

async fn set_autostart(
    State(state): State<AppState>,
    Json(input): Json<AutostartInput>,
) -> ApiResult {
    if input.enabled {
        crate::autostart::enable()?;
    } else {
        crate::autostart::disable()?;
    }
    state.settings.update(|settings| {
        settings.start_at_login = input.enabled;
    })?;
    ok(json!({ "startAtLogin": input.enabled }))
}

async fn setup_status(State(state): State<AppState>) -> ApiResult {
    ok(json!({
        "needed": !state.org.has_org()?,
        "timezones": [
            "Australia/Adelaide",
            "Australia/Brisbane",
            "Australia/Broken_Hill",
            "Australia/Darwin",
            "Australia/Eucla",
            "Australia/Hobart",
            "Australia/Lindeman",
            "Australia/Lord_Howe",
            "Australia/Melbourne",
            "Australia/Perth",
            "Australia/Sydney",
        ],
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetupInput {
    name: String,
    timezone: String,
    contact: String,
    actor_id: Option<String>,
    actor_name: Option<String>,
    device_name: String,
    discovery_preset: Option<String>,
    tailscale_listen: Option<String>,
}

async fn complete_setup(State(state): State<AppState>, Json(input): Json<SetupInput>) -> ApiResult {
    if state.org.has_org()? {
        return Err(ApiError(Error::Config(
            "This device already has an organisation.".into(),
        )));
    }
    let profile = state.org.init(
        &input.name,
        &input.timezone,
        &input.contact,
        input.actor_id.as_deref().unwrap_or("owner"),
        input.actor_name.as_deref().unwrap_or("Owner"),
    )?;
    if !input.device_name.trim().is_empty() {
        state
            .client
            .set_this_device_name(&input.device_name)
            .await?;
    }
    if let Some(preset) = input.discovery_preset {
        let preset = DiscoveryPreset::parse(&preset)?;
        state
            .client
            .apply_discovery(preset, input.tailscale_listen.as_deref())
            .await?;
        state.settings.update(|settings| {
            settings.discovery_preset = preset;
            if let Some(listen) = &input.tailscale_listen {
                settings.tailscale_listen = listen.clone();
            }
        })?;
    }
    ok(json!({ "organisation": profile, "setupNeeded": false }))
}

async fn org_profile(State(state): State<AppState>) -> ApiResult {
    ok(json!(state.org.get_org()?))
}

async fn list_members(State(state): State<AppState>) -> ApiResult {
    ok(json!({
        "members": state.org.list_members()?,
        "capabilities": state.org.capabilities(state.actor())?,
        "roleRule": "Only an owner can assign roles. An admin cannot create a new owner.",
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemberInput {
    id: String,
    name: String,
    role: String,
    device_id: Option<String>,
}

async fn add_member(State(state): State<AppState>, Json(input): Json<MemberInput>) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.assign_roles,
        "Only the owner can assign roles.",
    )?;
    let member = state.org.add_member(
        state.actor(),
        &input.id,
        &input.name,
        Role::from_str(&input.role)?,
        input.device_id.as_deref(),
    )?;
    created(json!(member))
}

#[derive(Deserialize)]
struct RoleInput {
    role: String,
}

async fn set_member_role(
    State(state): State<AppState>,
    AxumPath(member_id): AxumPath<String>,
    Json(input): Json<RoleInput>,
) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.assign_roles,
        "Only the owner can assign roles.",
    )?;
    let member = state
        .org
        .set_role(state.actor(), &member_id, Role::from_str(&input.role)?)?;
    ok(json!(member))
}

async fn audit_rows(State(state): State<AppState>) -> ApiResult {
    state.org.guard(
        state.actor(),
        |caps| caps.export_audit,
        "This role cannot read the audit log.",
    )?;
    ok(json!(state.org.list_audit(state.actor())?))
}

async fn audit_csv(State(state): State<AppState>) -> std::result::Result<Response, ApiError> {
    state.org.guard(
        state.actor(),
        |caps| caps.export_audit,
        "This role cannot export the audit log.",
    )?;
    let csv = state.org.export_audit_csv(state.actor())?;
    let mut response = csv.into_response();
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"blaksync-audit.csv\""),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

async fn office_health(State(state): State<AppState>) -> ApiResult {
    ok(json!(state.client.office_health().await?))
}

async fn version_check(State(state): State<AppState>) -> ApiResult {
    let settings = state.settings.load()?;
    ok(json!(
        updates::check_for_update(settings.version_check).await?
    ))
}

async fn privacy() -> Json<Value> {
    Json(json!({
        "title": "Privacy and support",
        "notice": PRIVACY_NOTICE,
        "support": "hello@yumait.com.au",
        "fileBytesStoredByYuma": false,
        "discoveryIsNotSecret": true,
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
    object.insert("unshareLeavesFiles".into(), json!(true));
    object.insert(
        "conflictAdvice".into(),
        json!("Keep both copies until a person chooses. BlakSync does not merge office documents."),
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
        let http_same = origin == &format!("http://{host_header}");
        let https_same = origin == &format!("https://{host_header}");
        let vite_origin = reqwest::Url::parse(origin).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some_and(is_loopback)
                && url.port() == Some(5173)
        });
        if !(http_same || vite_origin || (policy.tls && https_same)) {
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
            tls: false,
            actor: None,
        }
    }

    #[test]
    fn refuses_non_localhost_bind() {
        assert!(require_loopback_bind("0.0.0.0").is_err());
        assert!(require_loopback_bind("127.0.0.1").is_ok());
    }

    #[test]
    fn privacy_notice_is_honest() {
        let lower = PRIVACY_NOTICE.to_ascii_lowercase();
        assert!(lower.contains("file bytes"));
        assert!(lower.contains("does not run a cloud drive"));
        assert!(lower.contains("hello@yumait.com.au"));
        assert!(lower.contains("discovery"));
        assert!(
            !crate::org::Capabilities::from_member(&crate::org::Member {
                id: "field".into(),
                name: "Field".into(),
                role: Role::Member,
                device_id: None,
            })
            .accept
        );
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

    #[tokio::test]
    async fn member_cannot_accept_via_http() {
        let temporary = tempfile::tempdir().unwrap();
        let org = OrgOverlay::new(temporary.path().join("config")).unwrap();
        org.init(
            "Example Land Council",
            "Australia/Darwin",
            "it@example.org.au",
            "field-worker",
            "Field worker",
        )
        .unwrap();
        org.set_role(Some("field-worker"), "field-worker", Role::Owner)
            .unwrap();
        org.add_member(Some("field-worker"), "owner", "Owner", Role::Owner, None)
            .unwrap();
        // Make the local actor a member after adding a real owner.
        org.set_role(Some("field-worker"), "field-worker", Role::Member)
            .unwrap();
        let mut options = test_options(temporary.path());
        options.actor = Some("field-worker".into());
        let app = build_router(&options).unwrap();
        let response = app
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/pending/devices/AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH/accept")
                    .header(HOST, "127.0.0.1:8385")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}
