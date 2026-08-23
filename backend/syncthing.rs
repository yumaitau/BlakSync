use std::path::{Component, Path, PathBuf};

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::Method;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::org::require_device_id;
use crate::{Error, Result};

pub const DEFAULT_ORG_ROOT: &str = "/srv/blaksync";

#[derive(Clone, Debug)]
pub struct SyncthingClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatus {
    pub id: String,
    pub label: String,
    pub path: String,
    pub status: String,
    pub devices: Vec<String>,
    pub need_bytes: u64,
    pub need_files: u64,
    pub need_total_items: u64,
    pub out_of_sync: u64,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDeviceStatus {
    pub device_id: String,
    pub name: String,
    pub connected: bool,
    pub completion: f64,
    pub shared_folders: Vec<String>,
    pub paused: bool,
    pub address: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThisDevice {
    pub device_id: String,
    pub name: String,
    pub version: String,
    pub syncthing_gui_address: String,
    pub uptime_seconds: u64,
    pub inbound_bytes: u64,
    pub outbound_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderHealth {
    pub id: String,
    pub status: String,
    pub out_of_sync_items: u64,
    pub free_disk_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceHealth {
    pub device_id: String,
    pub connected: bool,
    pub last_seen: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OfficeHealth {
    pub folders: Vec<FolderHealth>,
    pub devices: Vec<DeviceHealth>,
}

impl SyncthingClient {
    pub fn new(base_url: Option<&str>, api_key: Option<&str>) -> Result<Self> {
        let api_key = api_key.filter(|value| !value.is_empty()).ok_or_else(|| {
            Error::Config("Set BLAKSYNC_API_KEY to your local Syncthing API key.".into())
        })?;
        Ok(Self {
            base_url: base_url
                .unwrap_or("http://127.0.0.1:8384")
                .trim_end_matches('/')
                .to_string(),
            api_key: api_key.to_string(),
            http: reqwest::Client::builder().build()?,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn request(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let method_name = method.as_str().to_string();
        let mut request = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header("X-API-Key", &self.api_key);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            let detail = text.trim();
            return Err(Error::Syncthing {
                message: format!(
                    "Syncthing {method_name} {path} failed ({}){}",
                    status.as_u16(),
                    if detail.is_empty() {
                        String::new()
                    } else {
                        format!(": {detail}")
                    }
                ),
                status: status.as_u16(),
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn system_status(&self) -> Result<Value> {
        self.request(Method::GET, "/rest/system/status", None).await
    }

    pub async fn device_id(&self) -> Result<String> {
        string_field(&self.system_status().await?, "myID")
    }

    pub async fn connections(&self) -> Result<Value> {
        self.request(Method::GET, "/rest/system/connections", None)
            .await
    }

    pub async fn devices(&self) -> Result<Vec<Value>> {
        array(
            self.request(Method::GET, "/rest/config/devices", None)
                .await?,
        )
    }

    pub async fn folders(&self) -> Result<Vec<Value>> {
        array(
            self.request(Method::GET, "/rest/config/folders", None)
                .await?,
        )
    }

    pub async fn pending_devices(&self) -> Result<Value> {
        self.request(Method::GET, "/rest/cluster/pending/devices", None)
            .await
    }

    pub async fn pending_folders(&self) -> Result<Value> {
        self.request(Method::GET, "/rest/cluster/pending/folders", None)
            .await
    }

    pub async fn gui_config(&self) -> Result<Value> {
        self.request(Method::GET, "/rest/config/gui", None).await
    }

    pub async fn add_device(&self, device_id: &str, name: &str) -> Result<Value> {
        let device_id = require_device_id(device_id)?;
        let mut device = object(
            self.request(Method::GET, "/rest/config/defaults/device", None)
                .await?,
        )?;
        device.insert("deviceID".into(), json!(device_id));
        device.insert("name".into(), json!(name));
        device.insert("addresses".into(), json!(["dynamic"]));
        let value = Value::Object(device);
        self.request(Method::POST, "/rest/config/devices", Some(value.clone()))
            .await?;
        Ok(value)
    }

    pub async fn deny_pending_device(&self, device_id: &str) -> Result<()> {
        let device_id = require_device_id(device_id)?;
        let path = format!(
            "/rest/cluster/pending/devices?device={}",
            encode(&device_id)
        );
        self.request(Method::DELETE, &path, None).await?;
        Ok(())
    }

    pub async fn add_folder(
        &self,
        folder_id: &str,
        path: &str,
        label: Option<&str>,
    ) -> Result<Value> {
        if folder_id.is_empty() || path.is_empty() {
            return Err(Error::Config("Folder ID and path are required".into()));
        }
        let mut folder = object(
            self.request(Method::GET, "/rest/config/defaults/folder", None)
                .await?,
        )?;
        folder.insert("id".into(), json!(folder_id));
        folder.insert("path".into(), json!(path));
        folder.insert("label".into(), json!(label.unwrap_or(folder_id)));
        folder.insert("type".into(), json!("sendreceive"));
        folder.insert("paused".into(), json!(false));
        let value = Value::Object(folder);
        self.request(Method::POST, "/rest/config/folders", Some(value.clone()))
            .await?;
        Ok(value)
    }

    pub async fn add_office_folder(
        &self,
        folder_id: &str,
        org_root: Option<&Path>,
        label: Option<&str>,
    ) -> Result<Value> {
        let root = org_root.unwrap_or_else(|| Path::new(DEFAULT_ORG_ROOT));
        let path = office_folder_path(folder_id, root)?;
        self.add_folder(folder_id, &path.to_string_lossy(), label)
            .await
    }

    pub async fn share_folder(&self, folder_id: &str, device_id: &str) -> Result<()> {
        let device_id = require_device_id(device_id)?;
        let path = format!("/rest/config/folders/{}", encode(folder_id));
        let folder = self.request(Method::GET, &path, None).await?;
        let mut devices = folder
            .get("devices")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !devices
            .iter()
            .any(|device| value_string(device, "deviceID") == device_id)
        {
            devices.push(json!({ "deviceID": device_id }));
            self.request(Method::PATCH, &path, Some(json!({ "devices": devices })))
                .await?;
        }
        Ok(())
    }

    pub async fn unshare_folder(&self, folder_id: &str, device_id: &str) -> Result<()> {
        let device_id = require_device_id(device_id)?;
        let path = format!("/rest/config/folders/{}", encode(folder_id));
        let folder = self.request(Method::GET, &path, None).await?;
        let devices: Vec<Value> = folder
            .get("devices")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|device| value_string(device, "deviceID") != device_id)
            .collect();
        self.request(Method::PATCH, &path, Some(json!({ "devices": devices })))
            .await?;
        Ok(())
    }

    pub async fn set_folder_paused(&self, folder_id: &str, paused: bool) -> Result<()> {
        let path = format!("/rest/config/folders/{}", encode(folder_id));
        self.request(Method::PATCH, &path, Some(json!({ "paused": paused })))
            .await?;
        Ok(())
    }

    pub async fn completion(
        &self,
        device_id: Option<&str>,
        folder_id: Option<&str>,
    ) -> Result<Value> {
        let mut query = Vec::new();
        if let Some(device_id) = device_id {
            query.push(format!("device={}", encode(device_id)));
        }
        if let Some(folder_id) = folder_id {
            query.push(format!("folder={}", encode(folder_id)));
        }
        self.request(
            Method::GET,
            &format!("/rest/db/completion?{}", query.join("&")),
            None,
        )
        .await
    }

    pub async fn folder_statuses(&self) -> Result<Vec<FolderStatus>> {
        let folders = self.folders().await?;
        let own_id = self.device_id().await?;
        let mut statuses = Vec::with_capacity(folders.len());
        for folder in folders {
            let remote_devices: Vec<String> = folder
                .get("devices")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|device| value_string(device, "deviceID"))
                .filter(|device| !device.is_empty() && device != &own_id)
                .collect();
            if bool_field(&folder, "paused") {
                statuses.push(folder_status(&folder, "Paused", remote_devices, None));
                continue;
            }
            if remote_devices.is_empty() {
                statuses.push(folder_status(&folder, "Unshared", remote_devices, None));
                continue;
            }
            let path = format!(
                "/rest/db/status?folder={}",
                encode(&value_string(&folder, "id"))
            );
            let runtime = self.request(Method::GET, &path, None).await?;
            let state = value_string(&runtime, "state");
            let status = if matches!(state.as_str(), "scanning" | "scan-wait" | "cleaning") {
                "Preparing"
            } else if state != "idle"
                || u64_field(&runtime, "needTotalItems") > 0
                || u64_field(&runtime, "needBytes") > 0
            {
                "Syncing"
            } else {
                "Up to Date"
            };
            statuses.push(folder_status(
                &folder,
                status,
                remote_devices,
                Some(&runtime),
            ));
        }
        Ok(statuses)
    }

    pub async fn remote_device_statuses(&self) -> Result<Vec<RemoteDeviceStatus>> {
        let devices = self.devices().await?;
        let connections = self.connections().await?;
        let own_id = self.device_id().await?;
        let folders = self.folders().await?;
        let connection_map = connections
            .get("connections")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let mut statuses = Vec::new();
        for device in devices {
            let device_id = value_string(&device, "deviceID");
            if device_id == own_id {
                continue;
            }
            let connection = connection_map
                .get(&device_id)
                .cloned()
                .unwrap_or(Value::Null);
            let completion = self
                .completion(Some(&device_id), None)
                .await
                .ok()
                .map(|value| f64_field(&value, "completion"))
                .unwrap_or_default();
            let shared_folders = folders
                .iter()
                .filter(|folder| {
                    folder
                        .get("devices")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .any(|entry| value_string(entry, "deviceID") == device_id)
                })
                .map(|folder| {
                    let label = value_string(folder, "label");
                    if label.is_empty() {
                        value_string(folder, "id")
                    } else {
                        label
                    }
                })
                .collect();
            let name = value_string(&device, "name");
            statuses.push(RemoteDeviceStatus {
                device_id: device_id.clone(),
                name: if name.is_empty() { device_id } else { name },
                connected: bool_field(&connection, "connected"),
                completion,
                shared_folders,
                paused: bool_field(&device, "paused"),
                address: value_string(&connection, "address"),
            });
        }
        Ok(statuses)
    }

    pub async fn this_device(&self) -> Result<ThisDevice> {
        let status = self.system_status().await?;
        let devices = self.devices().await?;
        let connections = self.connections().await?;
        let gui = self.gui_config().await?;
        let own_id = value_string(&status, "myID");
        let own = devices
            .iter()
            .find(|device| value_string(device, "deviceID") == own_id);
        let name = own
            .map(|device| value_string(device, "name"))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "This device".into());
        let total = connections.get("total").unwrap_or(&Value::Null);
        Ok(ThisDevice {
            device_id: own_id,
            name,
            version: value_string(&status, "version"),
            syncthing_gui_address: nonempty_or(value_string(&gui, "address"), "127.0.0.1:8384"),
            uptime_seconds: u64_field(&status, "uptime"),
            inbound_bytes: u64_field(total, "inBytesTotal"),
            outbound_bytes: u64_field(total, "outBytesTotal"),
        })
    }

    pub async fn office_health(&self) -> Result<OfficeHealth> {
        let folders = self.folder_statuses().await?;
        let device_stats = self
            .request(Method::GET, "/rest/stats/device", None)
            .await?;
        let connections = self.connections().await?;
        let own_id = self.device_id().await?;
        let connection_map = connections
            .get("connections")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let folder_health = folders
            .iter()
            .map(|folder| {
                Ok(FolderHealth {
                    id: folder.id.clone(),
                    status: folder.status.clone(),
                    out_of_sync_items: folder.need_total_items,
                    free_disk_bytes: fs2::available_space(Path::new(&folder.path))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let devices = device_stats
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(device_id, _)| *device_id != &own_id)
            .map(|(device_id, stats)| DeviceHealth {
                device_id: device_id.clone(),
                connected: connection_map
                    .get(device_id)
                    .is_some_and(|connection| bool_field(connection, "connected")),
                last_seen: stats
                    .get("lastSeen")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            })
            .collect();
        Ok(OfficeHealth {
            folders: folder_health,
            devices,
        })
    }
}

pub fn office_folder_path(folder_id: &str, org_root: &Path) -> Result<PathBuf> {
    if folder_id.is_empty()
        || !folder_id.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric() || (index > 0 && matches!(character, '.' | '_' | '-'))
        })
    {
        return Err(Error::Config(
            "Folder ID may only contain letters, numbers, dots, underscores, and hyphens".into(),
        ));
    }
    if !org_root.is_absolute() {
        return Err(Error::Config(
            "The organisation root must be an absolute path".into(),
        ));
    }
    if org_root
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(Error::Config(
            "Folder path must stay inside the organisation root".into(),
        ));
    }
    Ok(org_root.join("folders").join(folder_id))
}

fn folder_status(
    folder: &Value,
    status: &str,
    devices: Vec<String>,
    runtime: Option<&Value>,
) -> FolderStatus {
    let runtime = runtime.unwrap_or(&Value::Null);
    let id = value_string(folder, "id");
    FolderStatus {
        label: nonempty_or(value_string(folder, "label"), &id),
        id,
        path: value_string(folder, "path"),
        status: status.into(),
        devices,
        need_bytes: u64_field(runtime, "needBytes"),
        need_files: u64_field(runtime, "needFiles"),
        need_total_items: u64_field(runtime, "needTotalItems"),
        out_of_sync: if runtime.get("needTotalItems").is_some() {
            u64_field(runtime, "needTotalItems")
        } else {
            u64_field(runtime, "needFiles")
        },
        size_bytes: if runtime.get("globalBytes").is_some() {
            u64_field(runtime, "globalBytes")
        } else {
            u64_field(runtime, "localBytes")
        },
    }
}

fn array(value: Value) -> Result<Vec<Value>> {
    value
        .as_array()
        .cloned()
        .ok_or_else(|| Error::Config("Syncthing returned an unexpected JSON shape".into()))
}

fn object(value: Value) -> Result<Map<String, Value>> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| Error::Config("Syncthing returned an unexpected JSON shape".into()))
}

fn string_field(value: &Value, field: &str) -> Result<String> {
    let value = value_string(value, field);
    if value.is_empty() {
        return Err(Error::Config(format!(
            "Syncthing response did not contain {field}"
        )));
    }
    Ok(value)
}

fn value_string(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn bool_field(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn u64_field(value: &Value, field: &str) -> u64 {
    value.get(field).and_then(Value::as_u64).unwrap_or(0)
}

fn f64_field(value: &Value, field: &str) -> f64 {
    value.get(field).and_then(Value::as_f64).unwrap_or(0.0)
}

fn nonempty_or(value: String, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn encode(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::Json;
    use axum::Router;
    use axum::body::to_bytes;
    use axum::extract::{Request, State};
    use axum::http::StatusCode;
    use axum::routing::any;

    use super::*;

    const PEER: &str = "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH";

    #[derive(Clone, Debug)]
    struct Call {
        method: Method,
        uri: String,
        api_key: String,
        body: Value,
    }

    #[derive(Clone, Default)]
    struct FakeState {
        calls: Arc<Mutex<Vec<Call>>>,
    }

    async fn fake_syncthing(
        State(state): State<FakeState>,
        request: Request,
    ) -> (StatusCode, Json<Value>) {
        let method = request.method().clone();
        let uri = request.uri().to_string();
        let path = request.uri().path().to_string();
        let api_key = request
            .headers()
            .get("X-API-Key")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let body = to_bytes(request.into_body(), 1_048_576)
            .await
            .ok()
            .filter(|body| !body.is_empty())
            .and_then(|body| serde_json::from_slice(&body).ok())
            .unwrap_or(Value::Null);
        state.calls.lock().unwrap().push(Call {
            method: method.clone(),
            uri,
            api_key,
            body,
        });
        let response = match (method, path.as_str()) {
            (Method::GET, "/rest/config/defaults/device") => {
                json!({ "deviceID": "", "name": "", "addresses": [] })
            }
            (Method::GET, "/rest/config/folders/shared") => {
                json!({ "id": "shared", "devices": [] })
            }
            _ => Value::Null,
        };
        (StatusCode::OK, Json(response))
    }

    #[test]
    fn validates_office_folder_path() {
        assert_eq!(
            office_folder_path("country-docs", Path::new("/org-owned")).unwrap(),
            PathBuf::from("/org-owned/folders/country-docs")
        );
        assert!(office_folder_path("../escape", Path::new("/org-owned")).is_err());
        assert!(office_folder_path("valid", Path::new("relative")).is_err());
    }

    #[test]
    fn requires_api_key_without_echoing_it() {
        let error = SyncthingClient::new(None, None).unwrap_err().to_string();
        assert!(error.contains("BLAKSYNC_API_KEY"));
        assert!(!error.contains("secret"));
    }

    #[tokio::test]
    async fn sends_authenticated_syncthing_requests_with_expected_shapes() {
        let state = FakeState::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .fallback(any(fake_syncthing))
            .with_state(state.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client =
            SyncthingClient::new(Some(&format!("http://{address}")), Some("local-test-key"))
                .unwrap();
        client.add_device(PEER, "Field tablet").await.unwrap();
        client.share_folder("shared", PEER).await.unwrap();
        client.deny_pending_device(PEER).await.unwrap();
        server.abort();

        let calls = state.calls.lock().unwrap();
        assert_eq!(calls.len(), 5);
        assert!(calls.iter().all(|call| call.api_key == "local-test-key"));
        assert_eq!(calls[0].method, Method::GET);
        assert_eq!(calls[1].method, Method::POST);
        assert_eq!(calls[1].body["deviceID"], PEER);
        assert_eq!(calls[1].body["addresses"], json!(["dynamic"]));
        assert_eq!(calls[2].uri, "/rest/config/folders/shared");
        assert_eq!(calls[3].method, Method::PATCH);
        assert_eq!(calls[3].body["devices"][0]["deviceID"], PEER);
        assert_eq!(calls[4].method, Method::DELETE);
        assert!(
            calls[4]
                .uri
                .starts_with("/rest/cluster/pending/devices?device=")
        );
    }
}
