use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;
use std::sync::LazyLock;

use chrono::{SecondsFormat, Utc};
use chrono_tz::Tz;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::{ConfigStore, default_config_dir};
use crate::{Error, Result};

const ORG_FILE: &str = "org.json";
const ROLES_FILE: &str = "roles.json";
const FOLDERS_FILE: &str = "folders.json";
const PENDING_FILE: &str = "pending-devices.json";
const DEVICES_FILE: &str = "devices.json";
const AUDIT_FILE: &str = "audit.jsonl";

const MEMBER_CANNOT_ACCEPT: &str = "Members cannot accept a new device.";

static SIMPLE_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9._-]+$").expect("valid id regex"));
static DEVICE_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:[A-Z2-7]{7}-){7}[A-Z2-7]{7}$").expect("valid device id regex")
});

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OrgProfile {
    pub name: String,
    pub timezone: String,
    pub contact: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Admin,
    Member,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

impl FromStr for Role {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "owner" => Ok(Self::Owner),
            "admin" => Ok(Self::Admin),
            "member" => Ok(Self::Member),
            _ => Err(Error::Config(
                "Role must be one of: owner, admin, member.".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub device_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct RolesFile {
    local_member_id: Option<String>,
    members: Vec<Member>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FolderRecord {
    id: String,
    label: String,
    org_name: String,
    #[serde(default)]
    access_note: String,
    #[serde(default = "pair_roles")]
    who_may_pair: Vec<Role>,
    path: Option<String>,
    #[serde(default)]
    shared_with: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct FoldersFile {
    folders: Vec<FolderRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FolderView {
    pub id: String,
    pub label: String,
    pub org_name: String,
    pub access_note: String,
    pub who_may_pair: Vec<Role>,
    pub shared_with: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PendingDevice {
    device_id: String,
    name: String,
    recorded_at: String,
    #[serde(default)]
    folder_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct PendingFile {
    devices: Vec<PendingDevice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AcceptedDevice {
    device_id: String,
    name: String,
    accepted_at: String,
    accepted_by: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DevicesFile {
    devices: Vec<AcceptedDevice>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PendingPrompt {
    pub device_id: String,
    pub device_name: String,
    pub organisation: String,
    pub actor_id: String,
    pub actor_role: Role,
    pub can_accept: bool,
    pub refusal_reason: Option<String>,
    pub folders: Vec<PromptFolder>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PromptFolder {
    pub id: String,
    pub label: String,
    pub org_name: String,
    pub access_note: String,
    pub who_may_pair: Vec<Role>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AcceptResult {
    pub accepted: bool,
    pub device_id: String,
    pub prompt: PendingPrompt,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuditRow {
    pub timestamp: String,
    pub event: String,
    pub actor: String,
    pub role: Role,
    pub device_id: String,
    pub folder_label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub actor_id: String,
    pub actor_name: String,
    pub role: Option<Role>,
    pub accept: bool,
    pub share: bool,
    pub revoke: bool,
    pub assign_roles: bool,
    pub export_audit: bool,
    pub write_folder: bool,
    pub edit_settings: bool,
}

impl Capabilities {
    pub fn unrestricted() -> Self {
        Self {
            actor_id: String::new(),
            actor_name: String::new(),
            role: None,
            accept: true,
            share: true,
            revoke: true,
            assign_roles: true,
            export_audit: true,
            write_folder: true,
            edit_settings: true,
        }
    }

    pub fn from_member(actor: &Member) -> Self {
        Self {
            actor_id: actor.id.clone(),
            actor_name: actor.name.clone(),
            role: Some(actor.role),
            accept: allowed(actor.role, Action::AcceptDevice),
            share: allowed(actor.role, Action::ShareFolder),
            revoke: allowed(actor.role, Action::RevokeDevice),
            assign_roles: allowed(actor.role, Action::AssignRole),
            export_audit: allowed(actor.role, Action::ExportAudit),
            write_folder: allowed(actor.role, Action::WriteFolder),
            edit_settings: allowed(actor.role, Action::SetOrgProfile),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OrgOverlay {
    store: ConfigStore,
}

impl OrgOverlay {
    pub fn new(config_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: ConfigStore::new(config_dir)?,
        })
    }

    pub fn from_default() -> Result<Self> {
        Self::new(default_config_dir())
    }

    pub fn config_dir(&self) -> &Path {
        self.store.root()
    }

    pub fn init(
        &self,
        name: &str,
        timezone: &str,
        contact: &str,
        actor_id: &str,
        actor_name: &str,
    ) -> Result<OrgProfile> {
        self.store.with_lock(|| {
            if self.store.exists(ORG_FILE) {
                return Err(Error::Config(
                    "This config directory already has an organisation.".into(),
                ));
            }
            let profile = OrgProfile {
                name: required_text(name, "Organisation name")?,
                timezone: validate_timezone(timezone)?,
                contact: required_text(contact, "Contact")?,
            };
            let actor_id = require_simple_id(actor_id, "Member id")?;
            let roles = RolesFile {
                local_member_id: Some(actor_id.clone()),
                members: vec![Member {
                    id: actor_id,
                    name: required_text(actor_name, "Actor name")?,
                    role: Role::Owner,
                    device_id: None,
                }],
            };
            self.store.write_json(ORG_FILE, &profile)?;
            self.store.write_json(ROLES_FILE, &roles)?;
            self.store
                .write_json(FOLDERS_FILE, &FoldersFile::default())?;
            self.store
                .write_json(PENDING_FILE, &PendingFile::default())?;
            self.store
                .write_json(DEVICES_FILE, &DevicesFile::default())?;
            Ok(profile)
        })
    }

    pub fn get_org(&self) -> Result<OrgProfile> {
        self.store.with_lock(|| self.require_org())
    }

    pub fn has_org(&self) -> Result<bool> {
        self.store.with_lock(|| Ok(self.store.exists(ORG_FILE)))
    }

    pub fn local_actor(&self, actor_id: Option<&str>) -> Result<Option<Member>> {
        self.store.with_lock(|| {
            if !self.store.exists(ORG_FILE) {
                return Ok(None);
            }
            Ok(Some(self.actor(actor_id)?))
        })
    }

    pub fn capabilities(&self, actor_id: Option<&str>) -> Result<Capabilities> {
        match self.local_actor(actor_id)? {
            Some(actor) => Ok(Capabilities::from_member(&actor)),
            None => Ok(Capabilities::unrestricted()),
        }
    }

    pub fn register_device(
        &self,
        actor_id: Option<&str>,
        device_id: &str,
        name: &str,
    ) -> Result<()> {
        if !self.has_org()? {
            return Ok(());
        }
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            let org = self.require_org()?;
            let device_id = require_device_id(device_id)?;
            let mut pending = self.read_pending()?;
            pending
                .devices
                .retain(|device| device.device_id != device_id);
            self.store.write_json(PENDING_FILE, &pending)?;
            let mut devices = self.read_devices()?;
            if !devices
                .devices
                .iter()
                .any(|device| device.device_id == device_id)
            {
                devices.devices.push(AcceptedDevice {
                    device_id: device_id.clone(),
                    name: if name.trim().is_empty() {
                        device_id.clone()
                    } else {
                        name.trim().to_string()
                    },
                    accepted_at: timestamp(&org.timezone)?,
                    accepted_by: actor.id.clone(),
                });
                self.store.write_json(DEVICES_FILE, &devices)?;
            }
            Ok(())
        })
    }

    pub fn revoke_device(&self, actor_id: Option<&str>, device_id: &str) -> Result<Vec<String>> {
        self.store.with_lock(|| {
            if !self.store.exists(ORG_FILE) {
                return Ok(Vec::new());
            }
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::RevokeDevice)?;
            let org = self.require_org()?;
            let device_id = require_device_id(device_id)?;
            let mut folders = self.read_folders()?;
            let mut unshared = Vec::new();
            for folder in &mut folders.folders {
                if folder.shared_with.iter().any(|item| item == &device_id) {
                    folder.shared_with.retain(|item| item != &device_id);
                    unshared.push(folder.label.clone());
                    self.audit(&org, "folder_unshared", &actor, &device_id, &folder.label)?;
                }
            }
            self.store.write_json(FOLDERS_FILE, &folders)?;
            let mut devices = self.read_devices()?;
            devices
                .devices
                .retain(|device| device.device_id != device_id);
            self.store.write_json(DEVICES_FILE, &devices)?;
            let mut pending = self.read_pending()?;
            pending
                .devices
                .retain(|device| device.device_id != device_id);
            self.store.write_json(PENDING_FILE, &pending)?;
            self.audit(&org, "device_revoked", &actor, &device_id, "")?;
            Ok(unshared)
        })
    }

    pub fn set_org(
        &self,
        actor_id: Option<&str>,
        name: Option<&str>,
        timezone: Option<&str>,
        contact: Option<&str>,
    ) -> Result<OrgProfile> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            let mut org = self.require_org()?;
            if let Some(name) = name {
                require_permission(&actor, Action::SetOrgName)?;
                org.name = required_text(name, "Organisation name")?;
            }
            if timezone.is_some() || contact.is_some() {
                require_permission(&actor, Action::SetOrgProfile)?;
            }
            if let Some(timezone) = timezone {
                org.timezone = validate_timezone(timezone)?;
            }
            if let Some(contact) = contact {
                org.contact = required_text(contact, "Contact")?;
            }
            self.store.write_json(ORG_FILE, &org)?;
            let mut folders = self.read_folders()?;
            for folder in &mut folders.folders {
                folder.org_name.clone_from(&org.name);
            }
            self.store.write_json(FOLDERS_FILE, &folders)?;
            Ok(org)
        })
    }

    pub fn list_members(&self) -> Result<Vec<Member>> {
        self.store.with_lock(|| {
            self.require_org()?;
            Ok(self.read_roles()?.members)
        })
    }

    pub fn add_member(
        &self,
        actor_id: Option<&str>,
        member_id: &str,
        name: &str,
        role: Role,
        device_id: Option<&str>,
    ) -> Result<Member> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::AssignRole)?;
            let mut roles = self.read_roles()?;
            let member_id = require_simple_id(member_id, "Member id")?;
            if roles.members.iter().any(|member| member.id == member_id) {
                return Err(Error::Config(format!("Member already exists: {member_id}")));
            }
            if actor.role != Role::Owner && role == Role::Owner {
                return Err(Error::Role(
                    "Only an owner can create or promote another owner.".into(),
                ));
            }
            let member = Member {
                id: member_id,
                name: required_text(name, "Member name")?,
                role,
                device_id: optional_device_id(device_id)?,
            };
            roles.members.push(member.clone());
            self.store.write_json(ROLES_FILE, &roles)?;
            let org = self.require_org()?;
            self.audit(&org, "role_changed", &actor, "", &member.name)?;
            Ok(member)
        })
    }

    pub fn set_role(&self, actor_id: Option<&str>, member_id: &str, role: Role) -> Result<Member> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::AssignRole)?;
            if actor.role != Role::Owner && role == Role::Owner {
                return Err(Error::Role(
                    "Only an owner can create or promote another owner.".into(),
                ));
            }
            let mut roles = self.read_roles()?;
            let owner_count = roles
                .members
                .iter()
                .filter(|member| member.role == Role::Owner)
                .count();
            let member = roles
                .members
                .iter_mut()
                .find(|member| member.id == member_id)
                .ok_or_else(|| Error::NotFound(format!("Unknown member: {member_id}")))?;
            if member.role == Role::Owner && role != Role::Owner && owner_count == 1 {
                return Err(Error::Config(
                    "The last owner cannot demote themselves. The organisation must keep at least one owner.".into(),
                ));
            }
            member.role = role;
            let result = member.clone();
            self.store.write_json(ROLES_FILE, &roles)?;
            let org = self.require_org()?;
            self.audit(&org, "role_changed", &actor, "", &result.name)?;
            Ok(result)
        })
    }

    pub fn add_folder(
        &self,
        actor_id: Option<&str>,
        folder_id: &str,
        label: &str,
        access_note: &str,
        who_may_pair: Option<Vec<Role>>,
        path: Option<&str>,
    ) -> Result<FolderView> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::WriteFolder)?;
            let org = self.require_org()?;
            let mut folders = self.read_folders()?;
            let folder_id = require_simple_id(folder_id, "Folder id")?;
            if folders.folders.iter().any(|folder| folder.id == folder_id) {
                return Err(Error::Config(format!("Folder already exists: {folder_id}")));
            }
            let who_may_pair = validate_pair_roles(who_may_pair)?;
            let record = FolderRecord {
                id: folder_id,
                label: required_text(label, "Folder label")?,
                org_name: org.name,
                access_note: access_note.to_string(),
                who_may_pair,
                path: path
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_string),
                shared_with: Vec::new(),
            };
            let view = folder_view(&record);
            folders.folders.push(record);
            self.store.write_json(FOLDERS_FILE, &folders)?;
            Ok(view)
        })
    }

    pub fn set_access_note(
        &self,
        actor_id: Option<&str>,
        folder_id: &str,
        access_note: &str,
    ) -> Result<FolderView> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::WriteFolder)?;
            let mut folders = self.read_folders()?;
            let folder = find_folder_mut(&mut folders, folder_id)?;
            folder.access_note = access_note.to_string();
            let view = folder_view(folder);
            self.store.write_json(FOLDERS_FILE, &folders)?;
            Ok(view)
        })
    }

    pub fn list_folders(&self) -> Result<Vec<FolderView>> {
        self.store.with_lock(|| {
            self.require_org()?;
            Ok(self
                .read_folders()?
                .folders
                .iter()
                .map(folder_view)
                .collect())
        })
    }

    pub fn add_pending_device(
        &self,
        actor_id: Option<&str>,
        device_id: &str,
        name: &str,
        folder_ids: Vec<String>,
    ) -> Result<serde_json::Value> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::RecordPendingDevice)?;
            let org = self.require_org()?;
            let mut pending = self.read_pending()?;
            let device_id = require_device_id(device_id)?;
            if pending
                .devices
                .iter()
                .any(|device| device.device_id == device_id)
            {
                return Err(Error::Config(format!(
                    "Device is already pending: {device_id}"
                )));
            }
            if self
                .read_devices()?
                .devices
                .iter()
                .any(|device| device.device_id == device_id)
            {
                return Err(Error::Config(format!(
                    "Device is already accepted: {device_id}"
                )));
            }
            let folders = self.read_folders()?;
            for folder_id in &folder_ids {
                find_folder(&folders, folder_id)?;
            }
            let record = PendingDevice {
                device_id: device_id.clone(),
                name: if name.trim().is_empty() {
                    device_id.clone()
                } else {
                    name.trim().to_string()
                },
                recorded_at: timestamp(&org.timezone)?,
                folder_ids,
            };
            pending.devices.push(record.clone());
            self.store.write_json(PENDING_FILE, &pending)?;
            Ok(serde_json::to_value(record)?)
        })
    }

    pub fn list_pending(&self, actor_id: Option<&str>) -> Result<Vec<PendingPrompt>> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            self.read_pending()?
                .devices
                .iter()
                .map(|pending| self.prompt_unlocked(pending, &actor))
                .collect()
        })
    }

    pub fn pending_prompt(&self, actor_id: Option<&str>, device_id: &str) -> Result<PendingPrompt> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            let pending = self.read_pending()?;
            let device = find_pending(&pending, device_id)?;
            self.prompt_unlocked(device, &actor)
        })
    }

    pub fn accept_device(&self, actor_id: Option<&str>, device_id: &str) -> Result<AcceptResult> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            let mut pending = self.read_pending()?;
            let pending_device = find_pending(&pending, device_id)?.clone();
            let prompt = self.prompt_unlocked(&pending_device, &actor)?;
            require_permission(&actor, Action::AcceptDevice)?;
            let org = self.require_org()?;
            let mut devices = self.read_devices()?;
            devices.devices.push(AcceptedDevice {
                device_id: pending_device.device_id.clone(),
                name: pending_device.name,
                accepted_at: timestamp(&org.timezone)?,
                accepted_by: actor.id.clone(),
            });
            pending
                .devices
                .retain(|device| device.device_id != pending_device.device_id);
            self.store.write_json(DEVICES_FILE, &devices)?;
            self.store.write_json(PENDING_FILE, &pending)?;
            self.audit(
                &org,
                "device_accepted",
                &actor,
                &pending_device.device_id,
                "",
            )?;
            Ok(AcceptResult {
                accepted: true,
                device_id: pending_device.device_id,
                prompt,
            })
        })
    }

    pub fn deny_device(
        &self,
        actor_id: Option<&str>,
        device_id: &str,
    ) -> Result<serde_json::Value> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::DenyDevice)?;
            let mut pending = self.read_pending()?;
            find_pending(&pending, device_id)?;
            pending
                .devices
                .retain(|device| device.device_id != device_id);
            self.store.write_json(PENDING_FILE, &pending)?;
            Ok(serde_json::json!({ "denied": true, "device_id": device_id }))
        })
    }

    pub fn share_folder(
        &self,
        actor_id: Option<&str>,
        folder_id: &str,
        device_id: &str,
    ) -> Result<FolderView> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::ShareFolder)?;
            let org = self.require_org()?;
            let device_id = require_device_id(device_id)?;
            if !self
                .read_devices()?
                .devices
                .iter()
                .any(|device| device.device_id == device_id)
            {
                return Err(Error::NotFound(format!(
                    "Device has not been accepted: {device_id}"
                )));
            }
            let mut folders = self.read_folders()?;
            let folder = find_folder_mut(&mut folders, folder_id)?;
            if actor.role != Role::Owner && !folder.who_may_pair.contains(&actor.role) {
                return Err(Error::Role(format!(
                    "Role {} may not pair folder '{}'.",
                    actor.role.as_str(),
                    folder.label
                )));
            }
            if !folder.shared_with.contains(&device_id) {
                folder.shared_with.push(device_id.clone());
            }
            let label = folder.label.clone();
            let view = folder_view(folder);
            self.store.write_json(FOLDERS_FILE, &folders)?;
            self.audit(&org, "folder_shared", &actor, &device_id, &label)?;
            Ok(view)
        })
    }

    pub fn unshare_folder(
        &self,
        actor_id: Option<&str>,
        folder_id: &str,
        device_id: &str,
    ) -> Result<FolderView> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::ShareFolder)?;
            let org = self.require_org()?;
            let device_id = require_device_id(device_id)?;
            let mut folders = self.read_folders()?;
            let folder = find_folder_mut(&mut folders, folder_id)?;
            folder.shared_with.retain(|item| item != &device_id);
            let label = folder.label.clone();
            let view = folder_view(folder);
            self.store.write_json(FOLDERS_FILE, &folders)?;
            self.audit(&org, "folder_unshared", &actor, &device_id, &label)?;
            Ok(view)
        })
    }

    pub fn guard(
        &self,
        actor_id: Option<&str>,
        allowed: impl Fn(&Capabilities) -> bool,
        message: &str,
    ) -> Result<Capabilities> {
        let caps = self.capabilities(actor_id)?;
        if !allowed(&caps) {
            return Err(Error::Role(message.to_string()));
        }
        Ok(caps)
    }

    pub fn list_audit(&self, actor_id: Option<&str>) -> Result<Vec<AuditRow>> {
        self.store.with_lock(|| {
            let actor = self.actor(actor_id)?;
            require_permission(&actor, Action::ExportAudit)?;
            self.store.read_json_lines(AUDIT_FILE)
        })
    }

    pub fn export_audit_csv(&self, actor_id: Option<&str>) -> Result<String> {
        let rows = self.list_audit(actor_id)?;
        let mut writer = csv::WriterBuilder::new()
            .terminator(csv::Terminator::Any(b'\n'))
            .from_writer(Vec::new());
        writer.write_record([
            "timestamp",
            "event",
            "actor",
            "role",
            "device_id",
            "folder_label",
        ])?;
        for row in rows {
            writer.write_record([
                row.timestamp,
                row.event,
                row.actor,
                row.role.as_str().to_string(),
                row.device_id,
                row.folder_label,
            ])?;
        }
        let bytes = writer
            .into_inner()
            .map_err(|error| Error::Io(error.into_error()))?;
        String::from_utf8(bytes).map_err(|_| Error::Config("Audit CSV was not valid UTF-8.".into()))
    }

    fn require_org(&self) -> Result<OrgProfile> {
        self.store.read_optional_json(ORG_FILE)?.ok_or_else(|| {
            Error::NotFound("This config directory has no organisation yet. Run init first.".into())
        })
    }

    fn read_roles(&self) -> Result<RolesFile> {
        self.store.read_json(ROLES_FILE, RolesFile::default())
    }

    fn read_folders(&self) -> Result<FoldersFile> {
        self.store.read_json(FOLDERS_FILE, FoldersFile::default())
    }

    fn read_pending(&self) -> Result<PendingFile> {
        self.store.read_json(PENDING_FILE, PendingFile::default())
    }

    fn read_devices(&self) -> Result<DevicesFile> {
        self.store.read_json(DEVICES_FILE, DevicesFile::default())
    }

    fn actor(&self, actor_id: Option<&str>) -> Result<Member> {
        let roles = self.read_roles()?;
        let member_id = actor_id
            .map(str::to_string)
            .or(roles.local_member_id.clone())
            .ok_or_else(|| {
                Error::Config("No local actor is set. Pass --actor or run init.".into())
            })?;
        roles
            .members
            .into_iter()
            .find(|member| member.id == member_id)
            .ok_or_else(|| Error::NotFound(format!("Unknown member: {member_id}")))
    }

    fn prompt_unlocked(&self, pending: &PendingDevice, actor: &Member) -> Result<PendingPrompt> {
        let org = self.require_org()?;
        let folders = self.read_folders()?;
        let chosen: Vec<&FolderRecord> = if pending.folder_ids.is_empty() {
            folders.folders.iter().collect()
        } else {
            pending
                .folder_ids
                .iter()
                .map(|id| find_folder(&folders, id))
                .collect::<Result<_>>()?
        };
        let prompt_folders: Vec<PromptFolder> = chosen
            .iter()
            .map(|folder| PromptFolder {
                id: folder.id.clone(),
                label: folder.label.clone(),
                org_name: folder.org_name.clone(),
                access_note: folder.access_note.clone(),
                who_may_pair: folder.who_may_pair.clone(),
            })
            .collect();
        let can_accept = actor.role != Role::Member && allowed(actor.role, Action::AcceptDevice);
        let text = render_prompt(&org.name, pending, actor.role, &prompt_folders);
        Ok(PendingPrompt {
            device_id: pending.device_id.clone(),
            device_name: pending.name.clone(),
            organisation: org.name,
            actor_id: actor.id.clone(),
            actor_role: actor.role,
            can_accept,
            refusal_reason: (!can_accept).then(|| MEMBER_CANNOT_ACCEPT.to_string()),
            folders: prompt_folders,
            text,
        })
    }

    fn audit(
        &self,
        org: &OrgProfile,
        event: &str,
        actor: &Member,
        device_id: &str,
        folder_label: &str,
    ) -> Result<()> {
        self.store.append_json_line(
            AUDIT_FILE,
            &AuditRow {
                timestamp: timestamp(&org.timezone)?,
                event: event.to_string(),
                actor: actor.id.clone(),
                role: actor.role,
                device_id: device_id.to_string(),
                folder_label: folder_label.to_string(),
            },
        )
    }
}

fn folder_view(folder: &FolderRecord) -> FolderView {
    FolderView {
        id: folder.id.clone(),
        label: folder.label.clone(),
        org_name: folder.org_name.clone(),
        access_note: folder.access_note.clone(),
        who_may_pair: folder.who_may_pair.clone(),
        shared_with: folder.shared_with.clone(),
    }
}

fn render_prompt(
    org_name: &str,
    pending: &PendingDevice,
    actor_role: Role,
    folders: &[PromptFolder],
) -> String {
    let mut lines = vec![
        "Pending device".to_string(),
        "--------------".to_string(),
        format!("Organisation: {org_name}"),
        format!("Device name: {}", pending.name),
        format!("Device ID: {}", pending.device_id),
        String::new(),
        "Read the access notes before you accept this device.".to_string(),
        "The organisation writes these notes. BlakSync does not invent ceremony rules.".to_string(),
        String::new(),
    ];
    if folders.is_empty() {
        lines.push("No folders are listed for this pairing yet.".into());
        lines.push(String::new());
    } else {
        lines.push("Folders:".into());
        for folder in folders {
            lines.push(format!("  {}", folder.label));
            let roles = folder
                .who_may_pair
                .iter()
                .map(|role| role.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("    Who may pair: {roles}"));
            let note = if folder.access_note.trim().is_empty() {
                "(No access note has been written yet.)"
            } else {
                &folder.access_note
            };
            lines.push(format!("    Access note: {note}"));
            lines.push(String::new());
        }
    }
    lines.push(format!("Your role: {}", actor_role.as_str()));
    if actor_role == Role::Member {
        lines.push("You cannot accept this device. A member cannot accept a new device.".into());
    } else {
        lines.push(
            "Accepting this device lets it connect. Share folders separately after you accept."
                .into(),
        );
    }
    format!("{}\n", lines.join("\n").trim_end())
}

fn required_text(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(Error::Config(format!("{label} is required.")));
    }
    Ok(value.to_string())
}

fn require_simple_id(value: &str, label: &str) -> Result<String> {
    let value = required_text(value, label)?;
    if !SIMPLE_ID.is_match(&value) {
        return Err(Error::Config(format!(
            "{} may contain letters, numbers, dot, underscore, and hyphen.",
            label.to_ascii_lowercase()
        )));
    }
    Ok(value)
}

pub fn short_device_code(device_id: &str) -> String {
    device_id
        .chars()
        .filter(|character| *character != '-')
        .take(6)
        .collect::<String>()
        .to_ascii_uppercase()
}

pub fn looks_like_short_code(value: &str) -> bool {
    let value = value.trim();
    value.len() == 6
        && value
            .chars()
            .all(|character| matches!(character, 'A'..='Z' | 'a'..='z' | '2'..='7'))
}

pub fn require_device_id(value: &str) -> Result<String> {
    let value = required_text(value, "Device ID")?;
    if !DEVICE_ID.is_match(&value) {
        return Err(Error::Config(
            "Device ID must be a full Syncthing device ID (eight groups of seven A-Z/2-7 characters)."
                .into(),
        ));
    }
    Ok(value)
}

fn optional_device_id(value: Option<&str>) -> Result<Option<String>> {
    match value.filter(|value| !value.trim().is_empty()) {
        Some(value) => Ok(Some(require_device_id(value)?)),
        None => Ok(None),
    }
}

fn validate_timezone(value: &str) -> Result<String> {
    let value = value.trim();
    if !value.starts_with("Australia/") {
        return Err(Error::Config(
            "Timezone must be an IANA name under Australia/ (for example Australia/Darwin).".into(),
        ));
    }
    Tz::from_str(value).map_err(|_| Error::Config(format!("Unknown timezone: {value}")))?;
    Ok(value.to_string())
}

fn timestamp(timezone: &str) -> Result<String> {
    let timezone = Tz::from_str(timezone)
        .map_err(|_| Error::Config(format!("Unknown timezone: {timezone}")))?;
    Ok(Utc::now()
        .with_timezone(&timezone)
        .to_rfc3339_opts(SecondsFormat::Secs, false))
}

fn validate_pair_roles(value: Option<Vec<Role>>) -> Result<Vec<Role>> {
    let roles = value
        .filter(|roles| !roles.is_empty())
        .unwrap_or_else(pair_roles);
    if roles.contains(&Role::Member) {
        return Err(Error::Config(
            "Members cannot pair a new device; who-may-pair may only include owner and admin."
                .into(),
        ));
    }
    let mut seen = HashSet::new();
    Ok(roles
        .into_iter()
        .filter(|role| seen.insert(*role))
        .collect())
}

fn pair_roles() -> Vec<Role> {
    vec![Role::Owner, Role::Admin]
}

fn find_folder<'a>(folders: &'a FoldersFile, folder_id: &str) -> Result<&'a FolderRecord> {
    folders
        .folders
        .iter()
        .find(|folder| folder.id == folder_id)
        .ok_or_else(|| Error::NotFound(format!("Unknown folder: {folder_id}")))
}

fn find_folder_mut<'a>(
    folders: &'a mut FoldersFile,
    folder_id: &str,
) -> Result<&'a mut FolderRecord> {
    folders
        .folders
        .iter_mut()
        .find(|folder| folder.id == folder_id)
        .ok_or_else(|| Error::NotFound(format!("Unknown folder: {folder_id}")))
}

fn find_pending<'a>(pending: &'a PendingFile, device_id: &str) -> Result<&'a PendingDevice> {
    pending
        .devices
        .iter()
        .find(|device| device.device_id == device_id)
        .ok_or_else(|| Error::NotFound(format!("No pending device: {device_id}")))
}

#[derive(Clone, Copy)]
enum Action {
    SetOrgName,
    SetOrgProfile,
    AssignRole,
    WriteFolder,
    AcceptDevice,
    DenyDevice,
    RecordPendingDevice,
    ShareFolder,
    ExportAudit,
    RevokeDevice,
}

fn allowed(role: Role, action: Action) -> bool {
    match action {
        Action::SetOrgName | Action::AssignRole => role == Role::Owner,
        Action::SetOrgProfile
        | Action::WriteFolder
        | Action::AcceptDevice
        | Action::DenyDevice
        | Action::RecordPendingDevice
        | Action::ShareFolder
        | Action::ExportAudit
        | Action::RevokeDevice => matches!(role, Role::Owner | Role::Admin),
    }
}

fn require_permission(actor: &Member, action: Action) -> Result<()> {
    if allowed(actor.role, action) {
        return Ok(());
    }
    let message = match action {
        Action::AcceptDevice => MEMBER_CANNOT_ACCEPT.to_string(),
        Action::ShareFolder => format!(
            "Role {} cannot share or unshare a folder.",
            actor.role.as_str()
        ),
        Action::WriteFolder => format!(
            "Role {} cannot change folders or access notes.",
            actor.role.as_str()
        ),
        Action::AssignRole => "Only the owner can assign roles.".into(),
        Action::SetOrgName => "Only the owner can change the organisation name.".into(),
        Action::SetOrgProfile => format!(
            "Role {} cannot change the organisation profile.",
            actor.role.as_str()
        ),
        Action::ExportAudit => format!("Role {} cannot export the audit log.", actor.role.as_str()),
        Action::RevokeDevice => format!("Role {} cannot revoke a device.", actor.role.as_str()),
        Action::DenyDevice | Action::RecordPendingDevice => format!(
            "Role {} cannot manage pending devices.",
            actor.role.as_str()
        ),
    };
    Err(Error::Role(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEER: &str = "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH";

    fn organisation(root: &Path) -> OrgOverlay {
        let org = OrgOverlay::new(root).unwrap();
        org.init(
            "Example Land Council",
            "Australia/Darwin",
            "it@example.org.au",
            "office-node",
            "Office node",
        )
        .unwrap();
        org
    }

    #[test]
    fn profile_roles_and_timezone_permissions() {
        let temporary = tempfile::tempdir().unwrap();
        let org = organisation(temporary.path());
        org.add_member(None, "it-admin", "IT admin", Role::Admin, None)
            .unwrap();
        org.add_member(None, "field-worker", "Field worker", Role::Member, None)
            .unwrap();
        assert_eq!(org.get_org().unwrap().name, "Example Land Council");
        assert!(
            org.set_org(Some("field-worker"), None, None, Some("x@example.org"))
                .is_err()
        );
        assert!(
            org.set_org(Some("it-admin"), Some("Renamed"), None, None)
                .is_err()
        );
        assert!(
            org.set_org(None, None, Some("Pacific/Auckland"), None)
                .is_err()
        );
        assert_eq!(
            org.set_org(None, None, Some("Australia/Sydney"), None)
                .unwrap()
                .timezone,
            "Australia/Sydney"
        );
    }

    #[test]
    fn prompt_hides_path_and_member_cannot_accept() {
        let temporary = tempfile::tempdir().unwrap();
        let org = organisation(temporary.path());
        org.add_member(None, "field-worker", "Field worker", Role::Member, None)
            .unwrap();
        org.add_folder(
            None,
            "heritage-scans",
            "Heritage scans",
            "Speak with the cultural officer first.",
            None,
            Some("/var/lib/blaksync/secret-heritage"),
        )
        .unwrap();
        org.add_pending_device(None, PEER, "Field tablet", vec!["heritage-scans".into()])
            .unwrap();
        let prompt = org.pending_prompt(Some("field-worker"), PEER).unwrap();
        assert!(
            prompt
                .text
                .contains("Speak with the cultural officer first.")
        );
        assert!(!prompt.text.contains("/var/lib"));
        assert!(!prompt.can_accept);
        assert!(org.accept_device(Some("field-worker"), PEER).is_err());
        assert_eq!(org.list_pending(None).unwrap().len(), 1);
    }

    #[test]
    fn accept_share_unshare_audit_and_isolation() {
        let temporary = tempfile::tempdir().unwrap();
        let first = organisation(&temporary.path().join("org-a"));
        let second = organisation(&temporary.path().join("org-b"));
        first
            .add_folder(
                None,
                "heritage",
                "Heritage scans",
                "Restricted.",
                None,
                None,
            )
            .unwrap();
        second
            .add_folder(None, "payroll", "Payroll", "Staff only.", None, None)
            .unwrap();
        first
            .add_pending_device(None, PEER, "Field tablet", vec!["heritage".into()])
            .unwrap();
        first.accept_device(None, PEER).unwrap();
        first.share_folder(None, "heritage", PEER).unwrap();
        first.unshare_folder(None, "heritage", PEER).unwrap();
        first
            .add_member(None, "it-admin", "IT admin", Role::Admin, None)
            .unwrap();
        first.set_role(None, "it-admin", Role::Member).unwrap();
        first.revoke_device(None, PEER).unwrap();
        let csv = first.export_audit_csv(None).unwrap();
        assert!(csv.contains("device_accepted"));
        assert!(csv.contains("role_changed"));
        assert!(csv.contains("device_revoked"));
        assert!(csv.contains("Heritage scans"));
        assert!(!csv.contains("path"));
        assert!(first.revoke_device(Some("it-admin"), PEER).is_err());
        assert_eq!(second.list_folders().unwrap()[0].label, "Payroll");
        assert!(
            !second
                .export_audit_csv(None)
                .unwrap()
                .contains("device_accepted")
        );
    }

    #[test]
    fn owner_only_folder_blocks_admin() {
        let temporary = tempfile::tempdir().unwrap();
        let org = organisation(temporary.path());
        org.add_member(None, "it-admin", "IT admin", Role::Admin, None)
            .unwrap();
        org.add_folder(
            None,
            "restricted",
            "Restricted collection",
            "Owner pairing only.",
            Some(vec![Role::Owner]),
            None,
        )
        .unwrap();
        org.add_pending_device(None, PEER, "Field tablet", vec![])
            .unwrap();
        org.accept_device(None, PEER).unwrap();
        assert!(
            org.share_folder(Some("it-admin"), "restricted", PEER)
                .is_err()
        );
        org.share_folder(None, "restricted", PEER).unwrap();
    }

    #[test]
    fn last_owner_cannot_demote_themselves() {
        let temporary = tempfile::tempdir().unwrap();
        let org = organisation(temporary.path());
        assert!(org.set_role(None, "office-node", Role::Member).is_err());
        assert!(
            !org.capabilities(Some("field-worker"))
                .unwrap_or_else(|_| {
                    org.add_member(None, "field-worker", "Field", Role::Member, None)
                        .unwrap();
                    org.capabilities(Some("field-worker")).unwrap()
                })
                .assign_roles
        );
        assert_eq!(short_device_code(PEER), "AAAAAA");
        assert!(looks_like_short_code("AAAAAA"));
    }
}
