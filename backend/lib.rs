pub mod app_settings;
pub mod autostart;
pub mod backup;
pub mod config;
pub mod error;
pub mod launcher;
pub mod notes;
pub mod org;
pub mod security_scan;
pub mod server;
pub mod syncthing;
pub mod tls;
pub mod tray;
pub mod updates;
pub mod version;

pub use error::{Error, Result};
