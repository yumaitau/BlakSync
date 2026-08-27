use std::path::{Path, PathBuf};
use std::str::FromStr;

use blaksync::config::{default_config_dir, resolve_api_key};
use blaksync::launcher::launch_syncthing;
use blaksync::notes::AccessNotesStore;
use blaksync::org::{OrgOverlay, Role};
use blaksync::server::{self, ServerOptions};
use blaksync::syncthing::SyncthingClient;
use blaksync::version::LONG_VERSION;
use blaksync::{Error, Result};
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};

#[derive(Debug, Parser)]
#[command(
    name = "blaksync",
    version = LONG_VERSION,
    about = "Local-first folder sync for Australian Indigenous organisations",
    long_about = "BlakSync combines a Rust control plane with Syncthing's proven peer-to-peer sync engine. Device trust and folder sharing remain explicit. Each organisation controls its own access notes. There is no social login, and government ID is not stored."
)]
struct Cli {
    /// Directory for local organisation metadata and access notes.
    #[arg(long, global = true, value_name = "DIR")]
    config_dir: Option<PathBuf>,
    /// Organisation member ID used for permission checks.
    #[arg(long, global = true, value_name = "MEMBER_ID")]
    actor: Option<String>,
    /// Print machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create the private Syncthing identity and run the sync engine.
    Start(StartArgs),
    /// Serve the BlakSync web application and local API.
    Gui(GuiArgs),
    /// Print this Syncthing device ID.
    DeviceId,
    /// List devices waiting for an explicit decision.
    PendingDevices,
    /// List configured Syncthing devices.
    Devices,
    /// Add a trusted Syncthing device.
    AddDevice(AddDeviceArgs),
    /// Deny a pending Syncthing device.
    DenyDevice(DeviceArgs),
    /// Add a folder to Syncthing.
    AddFolder(AddFolderArgs),
    /// Set the local access note for a folder.
    SetNote(NoteArgs),
    /// Create a folder under the organisation root.
    OfficeFolder(OfficeFolderArgs),
    /// Accept an offered folder and share it back to its device.
    AcceptFolder(AcceptFolderArgs),
    /// Share a Syncthing folder with one device.
    Share(FolderDeviceArgs),
    /// Stop sharing a Syncthing folder with one device.
    Unshare(FolderDeviceArgs),
    /// Pause a Syncthing folder.
    Pause(FolderArgs),
    /// Resume a Syncthing folder.
    Resume(FolderArgs),
    /// Show folder sync state.
    Status,
    /// Show office-oriented folder and device health.
    Health,
    /// Copy the office-node config onto org-owned disk.
    Backup(BackupArgs),
    /// Restore a config backup onto this machine.
    Restore(RestoreArgs),
    /// Start BlakSync at login, or stop that.
    Autostart(AutostartArgs),
    /// Unshare every folder and remove a lost device.
    Revoke(DeviceArgs),
    /// Run Syncthing and the GUI from a tray or user service.
    Tray,
    /// Manage the organisation profile, roles, access notes, and audit log.
    Org(OrgArgs),
}

#[derive(Debug, Args)]
struct StartArgs {
    #[arg(long, value_name = "PATH")]
    syncthing: Option<String>,
    #[arg(long, value_name = "DIR")]
    home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct GuiArgs {
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long, value_name = "DIR")]
    static_root: Option<PathBuf>,
    /// Serve HTTPS with a local certificate in the config directory.
    #[arg(long)]
    tls: bool,
}

#[derive(Debug, Args)]
struct BackupArgs {
    #[arg(long, value_name = "DIR")]
    out: PathBuf,
}

#[derive(Debug, Args)]
struct RestoreArgs {
    #[arg(long, value_name = "DIR")]
    from: PathBuf,
}

#[derive(Debug, Args)]
struct AutostartArgs {
    #[command(subcommand)]
    command: AutostartCommand,
}

#[derive(Debug, Subcommand)]
enum AutostartCommand {
    Enable,
    Disable,
    Status,
}

#[derive(Debug, Args)]
struct DeviceArgs {
    #[arg(long)]
    device: String,
}

#[derive(Debug, Args)]
struct AddDeviceArgs {
    #[arg(long)]
    device: String,
    #[arg(long, default_value = "")]
    name: String,
}

#[derive(Debug, Args)]
struct FolderArgs {
    #[arg(long)]
    folder: String,
}

#[derive(Debug, Args)]
struct FolderDeviceArgs {
    #[arg(long)]
    folder: String,
    #[arg(long)]
    device: String,
}

#[derive(Debug, Args)]
struct AddFolderArgs {
    #[arg(long)]
    folder: String,
    #[arg(long)]
    path: String,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Debug, Args)]
struct NoteArgs {
    #[arg(long)]
    folder: String,
    #[arg(long, default_value = "")]
    note: String,
}

#[derive(Debug, Args)]
struct OfficeFolderArgs {
    #[arg(long)]
    folder: String,
    #[arg(long)]
    label: Option<String>,
}

#[derive(Debug, Args)]
struct AcceptFolderArgs {
    #[arg(long)]
    folder: String,
    #[arg(long)]
    path: String,
    #[arg(long)]
    device: String,
    #[arg(long)]
    label: Option<String>,
}

#[derive(Debug, Args)]
struct OrgArgs {
    #[command(subcommand)]
    command: OrgCommand,
}

#[derive(Debug, Subcommand)]
enum OrgCommand {
    /// Create an organisation and its local owner.
    Init(OrgInitArgs),
    /// Show the organisation profile.
    Show,
    /// Update profile fields.
    Set(OrgSetArgs),
    /// List members and roles.
    Roles,
    /// Add a member (owner only).
    RoleAdd(RoleAddArgs),
    /// Change a member role (owner only).
    RoleSet(RoleSetArgs),
    /// List organisation folder metadata.
    Folders,
    /// Add organisation folder metadata.
    FolderAdd(OrgFolderAddArgs),
    /// Update a folder access note.
    FolderNote(OrgFolderNoteArgs),
    /// List pending devices with the organisation's access notes.
    Pending,
    /// Record a pending device.
    PendingAdd(PendingAddArgs),
    /// Accept a pending device (owner or admin).
    Accept(DeviceArgs),
    /// Deny a pending device (owner or admin).
    Deny(DeviceArgs),
    /// Record an organisation folder share.
    Share(FolderDeviceArgs),
    /// Record removal of an organisation folder share.
    Unshare(FolderDeviceArgs),
    /// Show the local audit log.
    Audit,
    /// Export the audit log as CSV without filesystem paths.
    AuditExport(AuditExportArgs),
}

#[derive(Debug, Args)]
struct OrgInitArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    timezone: String,
    #[arg(long)]
    contact: String,
    #[arg(long, default_value = "owner")]
    actor_id: String,
    #[arg(long, default_value = "Owner")]
    actor_name: String,
}

#[derive(Debug, Args)]
struct OrgSetArgs {
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    timezone: Option<String>,
    #[arg(long)]
    contact: Option<String>,
}

#[derive(Debug, Args)]
struct RoleAddArgs {
    #[arg(long = "id")]
    member_id: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    role: String,
    #[arg(long)]
    device: Option<String>,
}

#[derive(Debug, Args)]
struct RoleSetArgs {
    #[arg(long = "id")]
    member_id: String,
    #[arg(long)]
    role: String,
}

#[derive(Debug, Args)]
struct OrgFolderAddArgs {
    #[arg(long = "id")]
    folder_id: String,
    #[arg(long)]
    label: String,
    #[arg(long, default_value = "")]
    note: String,
    #[arg(long, value_name = "ROLES")]
    who_may_pair: Option<String>,
    #[arg(long)]
    path: Option<String>,
}

#[derive(Debug, Args)]
struct OrgFolderNoteArgs {
    #[arg(long = "id")]
    folder_id: String,
    #[arg(long)]
    note: String,
}

#[derive(Debug, Args)]
struct PendingAddArgs {
    #[arg(long)]
    device: String,
    #[arg(long, default_value = "")]
    name: String,
    #[arg(long)]
    folder: Vec<String>,
}

#[derive(Debug, Args)]
struct AuditExportArgs {
    #[arg(long)]
    out: Option<PathBuf>,
}

enum Output {
    Json(Value),
    Org(OrgOutputKind, Value),
    Pending(Value),
    Accepted(Value),
    Csv(String),
    None,
}

#[derive(Clone, Copy)]
enum OrgOutputKind {
    Profile,
    Members,
    Member,
    Folders,
    Folder,
    PendingDevice,
    Deny,
    Audit,
    AuditExport,
}

struct CliFailure {
    error: Error,
    prompt: Option<Value>,
}

impl From<Error> for CliFailure {
    fn from(error: Error) -> Self {
        Self {
            error,
            prompt: None,
        }
    }
}

type CliResult<T> = std::result::Result<T, CliFailure>;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let json_mode = cli.json;
    match run(cli).await {
        Ok(output) => print_output(output, json_mode),
        Err(failure) => {
            if json_mode {
                let mut payload = json!({ "error": failure.error.to_string() });
                if let Some(prompt) = failure.prompt {
                    payload["prompt"] = prompt;
                }
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).expect("JSON error value")
                );
            } else {
                if let Some(text) = failure
                    .prompt
                    .as_ref()
                    .and_then(|prompt| prompt.get("text"))
                    .and_then(Value::as_str)
                {
                    println!("{text}");
                }
                eprintln!("{}", failure.error);
            }
            std::process::exit(1);
        }
    }
}

async fn run(cli: Cli) -> CliResult<Output> {
    let config_dir = cli.config_dir.unwrap_or_else(default_config_dir);
    let actor = cli.actor.as_deref();
    match cli.command {
        Command::Start(args) => {
            let result = launch_syncthing(args.syncthing.as_deref(), args.home.as_deref()).await?;
            if result.exit_code != 0 {
                return Err(Error::Process(format!(
                    "Syncthing exited with status {}",
                    result.exit_code
                ))
                .into());
            }
            Ok(Output::None)
        }
        Command::Gui(args) => {
            let options = ServerOptions::from_environment(
                args.host,
                args.port,
                args.static_root,
                config_dir,
                args.tls,
            )?;
            server::serve(options).await?;
            Ok(Output::None)
        }
        Command::Backup(args) => {
            let path = blaksync::backup::backup(Some(&config_dir), &args.out)?;
            Ok(Output::Json(json!({
                "path": path,
                "notice": "Keep this backup on org-owned disk or a USB drive. Do not copy it into git."
            })))
        }
        Command::Restore(args) => {
            let path = blaksync::backup::restore(Some(&config_dir), &args.from)?;
            Ok(Output::Json(json!({ "path": path })))
        }
        Command::Autostart(args) => match args.command {
            AutostartCommand::Enable => {
                let path = blaksync::autostart::enable()?;
                Ok(Output::Json(json!({ "enabled": true, "path": path })))
            }
            AutostartCommand::Disable => {
                let removed = blaksync::autostart::disable()?;
                Ok(Output::Json(
                    json!({ "enabled": false, "removed": removed }),
                ))
            }
            AutostartCommand::Status => Ok(Output::Json(json!({
                "enabled": blaksync::autostart::status()?
            }))),
        },
        Command::Tray => {
            blaksync::tray::run_tray(config_dir, None).await?;
            Ok(Output::None)
        }
        Command::Org(args) => run_org(&config_dir, actor, args.command),
        command => Ok(run_syncthing(&config_dir, command).await?),
    }
}

async fn run_syncthing(config_dir: &Path, command: Command) -> Result<Output> {
    let client = syncthing_client(config_dir)?;
    let value = match command {
        Command::DeviceId => json!({ "deviceId": client.device_id().await? }),
        Command::PendingDevices => client.pending_devices().await?,
        Command::Devices => json!(client.devices().await?),
        Command::AddDevice(args) => client.add_device(&args.device, &args.name).await?,
        Command::DenyDevice(args) => {
            client.deny_pending_device(&args.device).await?;
            json!({ "denied": true })
        }
        Command::AddFolder(args) => {
            let folder = client
                .add_folder(&args.folder, &args.path, args.label.as_deref())
                .await?;
            if let Some(note) = args.note {
                AccessNotesStore::new(config_dir)?.set(&args.folder, &note)?;
            }
            folder
        }
        Command::SetNote(args) => json!({
            "accessNote": AccessNotesStore::new(config_dir)?.set(&args.folder, &args.note)?
        }),
        Command::OfficeFolder(args) => {
            let root = std::env::var_os("BLAKSYNC_ORG_ROOT").map(PathBuf::from);
            client
                .add_office_folder(&args.folder, root.as_deref(), args.label.as_deref())
                .await?
        }
        Command::AcceptFolder(args) => {
            let folder = client
                .add_folder(&args.folder, &args.path, args.label.as_deref())
                .await?;
            client.share_folder(&args.folder, &args.device).await?;
            folder
        }
        Command::Share(args) => {
            client.share_folder(&args.folder, &args.device).await?;
            json!({ "shared": true })
        }
        Command::Unshare(args) => {
            client.unshare_folder(&args.folder, &args.device).await?;
            json!({ "shared": false })
        }
        Command::Pause(args) => {
            client.set_folder_paused(&args.folder, true).await?;
            json!({ "paused": true })
        }
        Command::Resume(args) => {
            client.set_folder_paused(&args.folder, false).await?;
            json!({ "paused": false })
        }
        Command::Status => json!(client.folder_statuses().await?),
        Command::Health => json!(client.office_health().await?),
        Command::Revoke(args) => {
            client.revoke_device(&args.device).await?;
            if let Ok(org) = OrgOverlay::new(config_dir)
                && org.has_org()?
            {
                org.revoke_device(None, &args.device)?;
            }
            json!({
                "revoked": true,
                "notice": "The device is removed from every folder on this machine. Files already on the lost disk stay there. This is not a remote wipe."
            })
        }
        Command::Start(_)
        | Command::Gui(_)
        | Command::Org(_)
        | Command::Backup(_)
        | Command::Restore(_)
        | Command::Autostart(_)
        | Command::Tray => {
            return Err(Error::Config("Unsupported command routing".into()));
        }
    };
    Ok(Output::Json(value))
}

fn run_org(config_dir: &Path, actor: Option<&str>, command: OrgCommand) -> CliResult<Output> {
    let org = OrgOverlay::new(config_dir)?;
    let (kind, output) = match command {
        OrgCommand::Init(args) => (
            OrgOutputKind::Profile,
            json!(org.init(
                &args.name,
                &args.timezone,
                &args.contact,
                &args.actor_id,
                &args.actor_name,
            )?),
        ),
        OrgCommand::Show => (OrgOutputKind::Profile, json!(org.get_org()?)),
        OrgCommand::Set(args) => {
            if args.name.is_none() && args.timezone.is_none() && args.contact.is_none() {
                return Err(
                    Error::Config("Pass --name, --timezone, and/or --contact.".into()).into(),
                );
            }
            (
                OrgOutputKind::Profile,
                json!(org.set_org(
                    actor,
                    args.name.as_deref(),
                    args.timezone.as_deref(),
                    args.contact.as_deref(),
                )?),
            )
        }
        OrgCommand::Roles => (OrgOutputKind::Members, json!(org.list_members()?)),
        OrgCommand::RoleAdd(args) => (
            OrgOutputKind::Member,
            json!(org.add_member(
                actor,
                &args.member_id,
                &args.name,
                Role::from_str(&args.role)?,
                args.device.as_deref(),
            )?),
        ),
        OrgCommand::RoleSet(args) => (
            OrgOutputKind::Member,
            json!(org.set_role(actor, &args.member_id, Role::from_str(&args.role)?,)?),
        ),
        OrgCommand::Folders => (OrgOutputKind::Folders, json!(org.list_folders()?)),
        OrgCommand::FolderAdd(args) => (
            OrgOutputKind::Folder,
            json!(org.add_folder(
                actor,
                &args.folder_id,
                &args.label,
                &args.note,
                parse_roles(args.who_may_pair.as_deref())?,
                args.path.as_deref(),
            )?),
        ),
        OrgCommand::FolderNote(args) => (
            OrgOutputKind::Folder,
            json!(org.set_access_note(actor, &args.folder_id, &args.note)?),
        ),
        OrgCommand::Pending => return Ok(Output::Pending(json!(org.list_pending(actor)?))),
        OrgCommand::PendingAdd(args) => (
            OrgOutputKind::PendingDevice,
            json!(org.add_pending_device(actor, &args.device, &args.name, args.folder,)?),
        ),
        OrgCommand::Accept(args) => {
            let prompt = org.pending_prompt(actor, &args.device)?;
            match org.accept_device(actor, &args.device) {
                Ok(result) => return Ok(Output::Accepted(json!(result))),
                Err(error @ Error::Role(_)) => {
                    return Err(CliFailure {
                        error,
                        prompt: Some(json!(prompt)),
                    });
                }
                Err(error) => return Err(error.into()),
            }
        }
        OrgCommand::Deny(args) => (OrgOutputKind::Deny, org.deny_device(actor, &args.device)?),
        OrgCommand::Share(args) => (
            OrgOutputKind::Folder,
            json!(org.share_folder(actor, &args.folder, &args.device)?),
        ),
        OrgCommand::Unshare(args) => (
            OrgOutputKind::Folder,
            json!(org.unshare_folder(actor, &args.folder, &args.device)?),
        ),
        OrgCommand::Audit => (OrgOutputKind::Audit, json!(org.list_audit(actor)?)),
        OrgCommand::AuditExport(args) => {
            let csv = org.export_audit_csv(actor)?;
            if let Some(path) = args.out {
                std::fs::write(&path, &csv).map_err(Error::from)?;
                return Ok(Output::Org(
                    OrgOutputKind::AuditExport,
                    json!({ "path": path, "csv": csv }),
                ));
            }
            return Ok(Output::Csv(csv));
        }
    };
    Ok(Output::Org(kind, output))
}

fn syncthing_client(config_dir: &Path) -> Result<SyncthingClient> {
    let url = std::env::var("BLAKSYNC_URL").ok();
    let key = resolve_api_key(config_dir)?;
    SyncthingClient::new(url.as_deref(), Some(&key))
}

fn parse_roles(value: Option<&str>) -> Result<Option<Vec<Role>>> {
    value
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|role| !role.is_empty())
                .map(Role::from_str)
                .collect()
        })
        .transpose()
}

fn print_output(output: Output, json_mode: bool) {
    match output {
        Output::Json(value) => {
            if json_mode {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).expect("JSON value")
                );
            } else {
                print_human(&value);
            }
        }
        Output::Org(kind, value) => {
            if json_mode {
                print_json(&value);
            } else {
                print_org(kind, &value);
            }
        }
        Output::Pending(value) => {
            if json_mode {
                print_json(&value);
            } else if let Some(items) = value.as_array() {
                let blocks = items
                    .iter()
                    .filter_map(|item| item.get("text").and_then(Value::as_str))
                    .map(str::trim_end)
                    .collect::<Vec<_>>();
                println!("{}", blocks.join("\n\n"));
            }
        }
        Output::Accepted(value) => {
            if json_mode {
                print_json(&value);
            } else {
                if let Some(text) = value
                    .get("prompt")
                    .and_then(|prompt| prompt.get("text"))
                    .and_then(Value::as_str)
                {
                    println!("{text}");
                }
                if let Some(device_id) = value.get("device_id").and_then(Value::as_str) {
                    println!("Device accepted: {device_id}");
                }
            }
        }
        Output::Csv(value) => {
            if json_mode {
                print_json(&json!({ "csv": value }));
            } else {
                print!("{value}");
            }
        }
        Output::None => {}
    }
}

fn print_json(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("JSON value")
    );
}

fn print_human(value: &Value) {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        println!("{text}");
        return;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serializable JSON value")
    );
}

fn print_org(kind: OrgOutputKind, value: &Value) {
    match kind {
        OrgOutputKind::Profile => {
            println!("Organisation: {}", text_field(value, "name"));
            println!("Timezone: {}", text_field(value, "timezone"));
            println!("Contact: {}", text_field(value, "contact"));
        }
        OrgOutputKind::Members => {
            for member in value.as_array().into_iter().flatten() {
                print_member(member);
            }
        }
        OrgOutputKind::Member => print_member(value),
        OrgOutputKind::Folders => {
            let folders = value.as_array().map(Vec::as_slice).unwrap_or_default();
            if folders.is_empty() {
                println!("No folders in this organisation.");
            } else {
                for folder in folders {
                    print_folder(folder);
                    println!();
                }
            }
        }
        OrgOutputKind::Folder => print_folder(value),
        OrgOutputKind::PendingDevice => {
            println!(
                "Pending device recorded: {}",
                text_field(value, "device_id")
            );
        }
        OrgOutputKind::Deny => {
            println!("Device denied: {}", text_field(value, "device_id"));
        }
        OrgOutputKind::Audit => {
            let rows = value.as_array().map(Vec::as_slice).unwrap_or_default();
            if rows.is_empty() {
                println!("No audit events.");
            } else {
                for row in rows {
                    println!(
                        "{}  {}  actor={} role={}  device={}  folder={}",
                        text_field(row, "timestamp"),
                        text_field(row, "event"),
                        text_field(row, "actor"),
                        text_field(row, "role"),
                        nonempty_field(row, "device_id", "-"),
                        nonempty_field(row, "folder_label", "-"),
                    );
                }
            }
        }
        OrgOutputKind::AuditExport => {
            println!("Wrote audit CSV to {}", text_field(value, "path"));
        }
    }
}

fn print_member(member: &Value) {
    let device = member
        .get("device_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| format!("  device {value}"))
        .unwrap_or_default();
    println!(
        "{}  {}  {}{}",
        text_field(member, "id"),
        text_field(member, "name"),
        text_field(member, "role"),
        device,
    );
}

fn print_folder(folder: &Value) {
    println!("{}", text_field(folder, "label"));
    println!("  id: {}", text_field(folder, "id"));
    println!("  organisation: {}", text_field(folder, "org_name"));
    println!(
        "  who may pair: {}",
        string_list(folder, "who_may_pair").join(", ")
    );
    println!(
        "  access note: {}",
        nonempty_field(
            folder,
            "access_note",
            "(No access note has been written yet.)"
        )
    );
    let shared = string_list(folder, "shared_with");
    if !shared.is_empty() {
        println!("  shared with: {}", shared.join(", "));
    }
}

fn text_field<'a>(value: &'a Value, field: &str) -> &'a str {
    value.get(field).and_then(Value::as_str).unwrap_or_default()
}

fn nonempty_field<'a>(value: &'a Value, field: &str, fallback: &'a str) -> &'a str {
    let field = text_field(value, field);
    if field.is_empty() { fallback } else { field }
}

fn string_list<'a>(value: &'a Value, field: &str) -> Vec<&'a str> {
    value
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn help_keeps_the_product_safety_language() {
        let help = Cli::command().render_long_help().to_string().to_lowercase();
        assert!(help.contains("organisation"));
        assert!(help.contains("access notes"));
        assert!(help.contains("government id is not stored"));
        assert!(help.contains("no social login"));
        assert!(
            Cli::command()
                .get_version()
                .unwrap_or_default()
                .contains("2.1.3")
        );
    }

    #[test]
    fn parses_syncthing_share_without_org_ambiguity() {
        let cli = Cli::try_parse_from([
            "blaksync", "share", "--folder", "heritage", "--device", "peer",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::Share(_)));
    }

    #[test]
    fn parses_org_overlay_under_explicit_namespace() {
        let cli = Cli::try_parse_from([
            "blaksync",
            "--json",
            "org",
            "folder-add",
            "--id",
            "heritage",
            "--label",
            "Heritage scans",
        ])
        .unwrap();
        assert!(cli.json);
        assert!(matches!(
            cli.command,
            Command::Org(OrgArgs {
                command: OrgCommand::FolderAdd(_)
            })
        ));
    }
}
