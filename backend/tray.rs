use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::process::{Child, Command};
use tokio::time::sleep;

use crate::config::{default_config_dir, resolve_api_key};
use crate::launcher::spawn_syncthing;
use crate::syncthing::SyncthingClient;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayState {
    UpToDate,
    Syncing,
    Paused,
    NeedsYou,
}

impl TrayState {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::UpToDate => "Up to date",
            Self::Syncing => "Syncing",
            Self::Paused => "Paused",
            Self::NeedsYou => "Needs you",
        }
    }

    pub fn menu_text(self) -> String {
        format!("BlakSync: {}", self.as_label())
    }
}

pub fn classify_state(
    pending_devices: usize,
    folders_paused: bool,
    folders_syncing: bool,
) -> TrayState {
    if pending_devices > 0 {
        TrayState::NeedsYou
    } else if folders_paused {
        TrayState::Paused
    } else if folders_syncing {
        TrayState::Syncing
    } else {
        TrayState::UpToDate
    }
}

pub async fn run_tray(config_dir: PathBuf, syncthing: Option<String>) -> Result<()> {
    let (syncthing_child, prepared) =
        spawn_syncthing(syncthing.as_deref(), Some(&config_dir)).await?;
    wait_for_api(&prepared.home).await?;
    let gui_child = spawn_gui(&config_dir)?;
    println!("BlakSync tray is running. GUI: http://127.0.0.1:8385");
    println!("{}", TrayState::UpToDate.menu_text());

    #[cfg(any(windows, target_os = "macos"))]
    {
        native_tray(config_dir, syncthing_child, gui_child).await
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let mut syncthing_child = syncthing_child;
        let mut gui_child = gui_child;
        supervisor_loop(&config_dir, &mut syncthing_child, &mut gui_child).await
    }
}

async fn wait_for_api(home: &std::path::Path) -> Result<()> {
    for _ in 0..40 {
        if resolve_api_key(home).is_ok()
            && let Ok(client) = SyncthingClient::new(None, resolve_api_key(home).ok().as_deref())
            && client.device_id().await.is_ok()
        {
            return Ok(());
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(Error::Process(
        "Syncthing started but the local API was not ready.".into(),
    ))
}

fn spawn_gui(config_dir: &std::path::Path) -> Result<Child> {
    let exe = std::env::current_exe()?;
    Command::new(exe)
        .args(["--config-dir", &config_dir.display().to_string(), "gui"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(Error::from)
}

async fn current_state(config_dir: &std::path::Path) -> TrayState {
    let key = resolve_api_key(config_dir).ok();
    let Ok(client) = SyncthingClient::new(None, key.as_deref()) else {
        return TrayState::Paused;
    };
    let pending = client
        .pending_devices()
        .await
        .ok()
        .and_then(|value| value.as_object().map(|object| object.len()))
        .unwrap_or(0);
    let folders = client.folder_statuses().await.unwrap_or_default();
    let paused = !folders.is_empty() && folders.iter().all(|folder| folder.status == "Paused");
    let syncing = folders
        .iter()
        .any(|folder| folder.status == "Syncing" || folder.status == "Preparing");
    classify_state(pending, paused, syncing)
}

fn open_gui() {
    let url = "http://127.0.0.1:8385";
    let _ = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).status()
    } else if cfg!(windows) {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()
    } else {
        std::process::Command::new("xdg-open").arg(url).status()
    };
}

#[cfg(not(any(windows, target_os = "macos")))]
fn stop_children(syncthing: &mut Child, gui: &mut Child) {
    let _ = syncthing.start_kill();
    let _ = gui.start_kill();
}

#[cfg(not(any(windows, target_os = "macos")))]
async fn supervisor_loop(
    config_dir: &std::path::Path,
    syncthing: &mut Child,
    gui: &mut Child,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = sleep(Duration::from_secs(4)) => {
                let state = current_state(config_dir).await;
                println!("{}", state.menu_text());
            }
        }
    }
    stop_children(syncthing, gui);
    Ok(())
}

#[cfg(any(windows, target_os = "macos"))]
async fn native_tray(config_dir: PathBuf, syncthing: Child, gui: Child) -> Result<()> {
    let syncthing = Arc::new(Mutex::new(syncthing));
    let gui = Arc::new(Mutex::new(gui));
    tokio::task::spawn_blocking(move || native_tray_blocking(config_dir, syncthing, gui))
        .await
        .map_err(|error| Error::Process(format!("Tray thread failed: {error}")))?
}

#[cfg(any(windows, target_os = "macos"))]
fn native_tray_blocking(
    config_dir: PathBuf,
    syncthing: Arc<Mutex<Child>>,
    gui: Arc<Mutex<Child>>,
) -> Result<()> {
    use std::time::Instant;

    use tao::event_loop::{ControlFlow, EventLoop};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let menu = Menu::new();
    let status_item = MenuItem::new(TrayState::UpToDate.menu_text(), false, None);
    let open_item = MenuItem::new("Open BlakSync", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&status_item)
        .map_err(|error| Error::Process(error.to_string()))?;
    menu.append(&PredefinedMenuItem::separator())
        .map_err(|error| Error::Process(error.to_string()))?;
    menu.append(&open_item)
        .map_err(|error| Error::Process(error.to_string()))?;
    menu.append(&quit_item)
        .map_err(|error| Error::Process(error.to_string()))?;

    let mut pixels = Vec::with_capacity(16 * 16 * 4);
    for _ in 0..16 * 16 {
        pixels.extend_from_slice(&[50, 42, 32, 255]);
    }
    let icon = Icon::from_rgba(pixels, 16, 16)
        .map_err(|error| Error::Process(format!("Could not build tray icon: {error}")))?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("BlakSync: Up to date")
        .with_title("BlakSync: Up to date")
        .with_icon(icon)
        .build()
        .map_err(|error| Error::Process(format!("Could not show the tray icon: {error}")))?;

    let menu_channel = MenuEvent::receiver();
    let event_loop = EventLoop::new();
    let mut last_poll = Instant::now();
    let runtime = tokio::runtime::Handle::try_current().ok();
    event_loop.run(move |_, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_secs(2));
        if last_poll.elapsed() >= Duration::from_secs(3) {
            last_poll = Instant::now();
            let state = if let Some(handle) = &runtime {
                handle.block_on(current_state(&config_dir))
            } else {
                TrayState::UpToDate
            };
            let text = state.menu_text();
            status_item.set_text(text.clone());
            let _ = tray.set_tooltip(Some(text.clone()));
            tray.set_title(Some(text));
        }
        if let Ok(event) = menu_channel.try_recv() {
            if event.id == open_item.id() {
                open_gui();
            }
            if event.id == quit_item.id() {
                if let Ok(mut child) = syncthing.lock() {
                    let _ = child.start_kill();
                }
                if let Ok(mut child) = gui.lock() {
                    let _ = child.start_kill();
                }
                *control_flow = ControlFlow::Exit;
            }
        }
    })
}

pub fn default_tray_home() -> PathBuf {
    default_config_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_device_uses_needs_you_text() {
        assert_eq!(classify_state(1, false, false).as_label(), "Needs you");
        assert_eq!(classify_state(0, true, true).as_label(), "Paused");
        assert_eq!(classify_state(0, false, true).as_label(), "Syncing");
        assert_eq!(classify_state(0, false, false).as_label(), "Up to date");
        assert!(!TrayState::NeedsYou.menu_text().contains("red"));
    }
}
