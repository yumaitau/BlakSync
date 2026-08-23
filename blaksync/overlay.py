"""Org overlay: profile, roles, folder access notes, pending devices, audit."""

from __future__ import annotations

import os
import re
from pathlib import Path

from .audit import COLUMNS as AUDIT_COLUMNS
from .audit import audit_row, to_csv
from .errors import ConfigError, NotFoundError, RoleError
from .prompt import FolderView, PendingPrompt, render_pending_prompt
from .roles import MEMBER_CANNOT_ACCEPT, PAIR_ROLES, can, parse_role, parse_who_may_pair
from .store import ConfigStore
from .timeutil import stamp, validate_timezone

FOLDER_ID_RE = re.compile(r"^[A-Za-z0-9._-]+$")
DEVICE_ID_RE = re.compile(r"^(?:[A-Z2-7]{7}-){7}[A-Z2-7]{7}$")
MEMBER_ID_RE = re.compile(r"^[A-Za-z0-9._-]+$")

FORBIDDEN_PROFILE_KEYS = {
    "government_id",
    "governmentid",
    "gov_id",
    "govid",
    "passport",
    "passport_number",
    "medicare",
    "medicare_number",
    "tfn",
    "tax_file_number",
    "driver_licence",
    "drivers_license",
    "national_id",
    "birth_certificate",
}

ALLOWED_ORG_KEYS = {"name", "timezone", "contact"}


def default_config_dir():
    env = os.environ.get("BLAKSYNC_CONFIG_DIR")
    if env:
        return str(Path(env).expanduser())
    home = Path.home()
    if os.name == "nt":
        base = os.environ.get("APPDATA") or str(home / "AppData" / "Roaming")
        return str(Path(base) / "BlakSync")
    if sys_platform() == "darwin":
        return str(home / "Library" / "Application Support" / "BlakSync")
    xdg = os.environ.get("XDG_CONFIG_HOME")
    if xdg:
        return str(Path(xdg) / "blaksync")
    return str(home / ".config" / "blaksync")


def sys_platform():
    return os.environ.get("BLAKSYNC_FAKE_PLATFORM") or __import__("sys").platform


class BlakSyncOrg:
    """One organisation, bound to one config directory."""

    def __init__(self, config_dir=None):
        self.store = ConfigStore(config_dir or default_config_dir())

    @property
    def config_dir(self):
        return self.store.config_dir

    def init(self, *, name, timezone, contact, actor_id="owner", actor_name="Owner", **extra):
        with self.store.lock():
            if self.store.org_exists():
                raise ConfigError("This config directory already has an organisation.")
            profile = self._profile_from_args(
                name=name, timezone=timezone, contact=contact, extra=extra
            )
            member_id = _require_member_id(actor_id)
            member_name = _require_text(actor_name, "Actor name")
            self.store.write_org(profile)
            self.store.write_roles(
                {
                    "local_member_id": member_id,
                    "members": [
                        {
                            "id": member_id,
                            "name": member_name,
                            "role": "owner",
                            "device_id": None,
                        }
                    ],
                }
            )
            self.store.write_folders({"folders": []})
            self.store.write_pending({"devices": []})
            self.store.write_devices({"devices": []})
            return self._public_org(profile)

    def get_org(self):
        with self.store.lock():
            return self._public_org(self._require_org())

    def set_org(self, *, actor_id=None, name=None, timezone=None, contact=None, **extra):
        with self.store.lock():
            if extra:
                _reject_forbidden_keys(extra)
            actor = self._actor(actor_id)
            current = self._require_org()
            if name is not None:
                self._require_perm(actor, "org.set_name")
                current["name"] = _require_text(name, "Organisation name")
            if timezone is not None or contact is not None:
                self._require_perm(actor, "org.set_profile")
            if timezone is not None:
                current["timezone"] = _validate_timezone(timezone)
            if contact is not None:
                current["contact"] = _require_text(contact, "Contact")
            self.store.write_org(current)
            self._refresh_folder_org_names(current["name"])
            return self._public_org(current)

    def list_members(self):
        with self.store.lock():
            self._require_org()
            return [self._public_member(item) for item in self.store.read_roles()["members"]]

    def add_member(self, *, member_id, name, role, device_id=None, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "role.assign")
            roles = self.store.read_roles()
            new_id = _require_member_id(member_id)
            if any(item["id"] == new_id for item in roles["members"]):
                raise ConfigError(f"Member already exists: {new_id}")
            try:
                parsed_role = parse_role(role)
            except ValueError as exc:
                raise ConfigError(str(exc)) from exc
            device = _optional_device_id(device_id)
            roles["members"].append(
                {
                    "id": new_id,
                    "name": _require_text(name, "Member name"),
                    "role": parsed_role,
                    "device_id": device,
                }
            )
            self.store.write_roles(roles)
            return self._public_member(roles["members"][-1])

    def set_role(self, member_id, role, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "role.assign")
            roles = self.store.read_roles()
            member = _find_member(roles, member_id)
            try:
                parsed = parse_role(role)
            except ValueError as exc:
                raise ConfigError(str(exc)) from exc
            owners = [item for item in roles["members"] if item["role"] == "owner"]
            if member["role"] == "owner" and parsed != "owner" and len(owners) == 1:
                raise ConfigError("The organisation must keep at least one owner.")
            member["role"] = parsed
            self.store.write_roles(roles)
            return self._public_member(member)

    def set_local_actor(self, member_id):
        with self.store.lock():
            roles = self.store.read_roles()
            _find_member(roles, member_id)
            roles["local_member_id"] = member_id
            self.store.write_roles(roles)
            return self._public_member(_find_member(roles, member_id))

    def local_actor(self):
        with self.store.lock():
            return self._public_member(self._actor(None))

    def add_folder(
        self,
        *,
        folder_id,
        label,
        access_note="",
        who_may_pair=None,
        path=None,
        actor_id=None,
    ):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "folder.write")
            org = self._require_org()
            folders = self.store.read_folders()
            fid = _require_folder_id(folder_id)
            if any(item["id"] == fid for item in folders["folders"]):
                raise ConfigError(f"Folder already exists: {fid}")
            record = {
                "id": fid,
                "label": _require_text(label, "Folder label"),
                "org_name": org["name"],
                "access_note": _note(access_note),
                "who_may_pair": _who_may_pair(who_may_pair),
                "path": _optional_path(path),
                "shared_with": [],
            }
            folders["folders"].append(record)
            self.store.write_folders(folders)
            return self._public_folder(record)

    def set_access_note(self, folder_id, access_note, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "folder.write")
            folders = self.store.read_folders()
            folder = _find_folder(folders, folder_id)
            folder["access_note"] = _note(access_note)
            self.store.write_folders(folders)
            return self._public_folder(folder)

    def list_folders(self):
        with self.store.lock():
            self._require_org()
            return [self._public_folder(item) for item in self.store.read_folders()["folders"]]

    def add_pending_device(self, *, device_id, name="", folder_ids=None, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "device.pending_record")
            org = self._require_org()
            pending = self.store.read_pending()
            did = _require_device_id(device_id)
            if any(item["device_id"] == did for item in pending["devices"]):
                raise ConfigError(f"Device is already pending: {did}")
            accepted = self.store.read_devices()
            if any(item["device_id"] == did for item in accepted["devices"]):
                raise ConfigError(f"Device is already accepted: {did}")
            ids = list(folder_ids or [])
            for fid in ids:
                _find_folder(self.store.read_folders(), fid)
            record = {
                "device_id": did,
                "name": (name or did).strip(),
                "recorded_at": stamp(org["timezone"]),
                "folder_ids": ids,
            }
            pending["devices"].append(record)
            self.store.write_pending(pending)
            return dict(record)

    def list_pending(self, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            pending = self.store.read_pending()
            return [
                self._prompt_unlocked(item, actor).to_dict()
                for item in pending["devices"]
            ]

    def pending_prompt(self, device_id, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            pending = _find_pending(self.store.read_pending(), device_id)
            return self._prompt_unlocked(pending, actor)

    def accept_device(self, device_id, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            pending = _find_pending(self.store.read_pending(), device_id)
            prompt = self._prompt_unlocked(pending, actor)
            if actor["role"] == "member" or not can(actor["role"], "device.accept"):
                raise RoleError(MEMBER_CANNOT_ACCEPT, prompt=prompt)
            org = self._require_org()
            devices = self.store.read_devices()
            devices["devices"].append(
                {
                    "device_id": pending["device_id"],
                    "name": pending["name"],
                    "accepted_at": stamp(org["timezone"]),
                    "accepted_by": actor["id"],
                }
            )
            remaining = [
                item
                for item in self.store.read_pending()["devices"]
                if item["device_id"] != pending["device_id"]
            ]
            self.store.write_devices(devices)
            self.store.write_pending({"devices": remaining})
            self._audit(
                org,
                event="device_accepted",
                actor=actor,
                device_id=pending["device_id"],
            )
            return {"accepted": True, "device_id": pending["device_id"], "prompt": prompt.to_dict()}

    def deny_device(self, device_id, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "device.deny")
            pending = self.store.read_pending()
            _find_pending(pending, device_id)
            pending["devices"] = [
                item for item in pending["devices"] if item["device_id"] != device_id
            ]
            self.store.write_pending(pending)
            return {"denied": True, "device_id": device_id}

    def share_folder(self, folder_id, device_id, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "folder.share")
            org = self._require_org()
            folders = self.store.read_folders()
            folder = _find_folder(folders, folder_id)
            did = _require_device_id(device_id)
            _find_accepted(self.store.read_devices(), did)
            if actor["role"] != "owner" and actor["role"] not in folder["who_may_pair"]:
                raise RoleError(
                    f"Role {actor['role']} may not pair folder '{folder['label']}'."
                )
            if did not in folder["shared_with"]:
                folder["shared_with"].append(did)
            self.store.write_folders(folders)
            self._audit(
                org,
                event="folder_shared",
                actor=actor,
                device_id=did,
                folder_label=folder["label"],
            )
            return self._public_folder(folder)

    def unshare_folder(self, folder_id, device_id, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "folder.share")
            org = self._require_org()
            folders = self.store.read_folders()
            folder = _find_folder(folders, folder_id)
            did = _require_device_id(device_id)
            folder["shared_with"] = [item for item in folder["shared_with"] if item != did]
            self.store.write_folders(folders)
            self._audit(
                org,
                event="folder_unshared",
                actor=actor,
                device_id=did,
                folder_label=folder["label"],
            )
            return self._public_folder(folder)

    def list_audit(self, *, actor_id=None):
        with self.store.lock():
            actor = self._actor(actor_id)
            self._require_perm(actor, "audit.export")
            return list(self.store.read_audit())

    def export_audit_csv(self, *, actor_id=None):
        rows = self.list_audit(actor_id=actor_id)
        return to_csv(rows)

    def _prompt_unlocked(self, pending, actor):
        org = self._require_org()
        folders_data = self.store.read_folders()["folders"]
        requested = pending.get("folder_ids") or []
        if requested:
            chosen = [_find_folder({"folders": folders_data}, fid) for fid in requested]
        else:
            chosen = folders_data
        views = [
            FolderView(
                id=folder["id"],
                label=folder["label"],
                org_name=folder.get("org_name") or org["name"],
                access_note=folder.get("access_note") or "",
                who_may_pair=list(folder.get("who_may_pair") or PAIR_ROLES),
            )
            for folder in chosen
        ]
        can_accept = can(actor["role"], "device.accept") and actor["role"] != "member"
        reason = None if can_accept else MEMBER_CANNOT_ACCEPT
        text = render_pending_prompt(
            org_name=org["name"],
            device_id=pending["device_id"],
            device_name=pending["name"],
            actor_role=actor["role"],
            folders=views,
        )
        return PendingPrompt(
            device_id=pending["device_id"],
            device_name=pending["name"],
            organisation=org["name"],
            actor_id=actor["id"],
            actor_role=actor["role"],
            can_accept=can_accept,
            refusal_reason=reason,
            folders=views,
            text=text,
        )

    def _require_org(self):
        org = self.store.read_org()
        if not org:
            raise NotFoundError(
                "This config directory has no organisation yet. Run init first."
            )
        return org

    def _actor(self, actor_id):
        roles = self.store.read_roles()
        member_id = actor_id or roles.get("local_member_id")
        if not member_id:
            raise ConfigError("No local actor is set. Pass --actor or run init.")
        return _find_member(roles, member_id)

    def _require_perm(self, actor, action):
        if actor["role"] == "member" and action == "device.accept":
            raise RoleError(MEMBER_CANNOT_ACCEPT)
        if not can(actor["role"], action):
            raise RoleError(_permission_message(actor["role"], action))

    def _audit(self, org, *, event, actor, device_id="", folder_label=""):
        row = audit_row(
            timestamp=stamp(org["timezone"]),
            event=event,
            actor=actor["id"],
            role=actor["role"],
            device_id=device_id,
            folder_label=folder_label,
        )
        self.store.append_audit(row)

    def _refresh_folder_org_names(self, org_name):
        folders = self.store.read_folders()
        for folder in folders["folders"]:
            folder["org_name"] = org_name
        self.store.write_folders(folders)

    def _profile_from_args(self, *, name, timezone, contact, extra=None):
        if extra:
            _reject_forbidden_keys(extra)
        return {
            "name": _require_text(name, "Organisation name"),
            "timezone": _validate_timezone(timezone),
            "contact": _require_text(contact, "Contact"),
        }

    def _public_org(self, org):
        return {key: org[key] for key in ("name", "timezone", "contact")}

    def _public_member(self, member):
        return {
            "id": member["id"],
            "name": member["name"],
            "role": member["role"],
            "device_id": member.get("device_id"),
        }

    def _public_folder(self, folder):
        return {
            "id": folder["id"],
            "label": folder["label"],
            "org_name": folder.get("org_name"),
            "access_note": folder.get("access_note") or "",
            "who_may_pair": list(folder.get("who_may_pair") or PAIR_ROLES),
            "shared_with": list(folder.get("shared_with") or []),
        }


def _require_text(value, label):
    text = (value or "").strip()
    if not text:
        raise ConfigError(f"{label} is required.")
    return text


def _note(value):
    if value is None:
        return ""
    return str(value)


def _require_member_id(value):
    text = _require_text(value, "Member id")
    if not MEMBER_ID_RE.match(text):
        raise ConfigError("Member id may contain letters, numbers, dot, underscore, and hyphen.")
    return text


def _require_folder_id(value):
    text = _require_text(value, "Folder id")
    if not FOLDER_ID_RE.match(text):
        raise ConfigError("Folder id may contain letters, numbers, dot, underscore, and hyphen.")
    return text


def _require_device_id(value):
    text = _require_text(value, "Device ID")
    if not DEVICE_ID_RE.match(text):
        raise ConfigError(
            "Device ID must be a full Syncthing device ID (eight groups of seven A-Z/2-7 characters)."
        )
    return text


def _optional_device_id(value):
    if value is None or str(value).strip() == "":
        return None
    return _require_device_id(value)


def _optional_path(value):
    if value is None or str(value).strip() == "":
        return None
    return str(value)


def _who_may_pair(value):
    try:
        return parse_who_may_pair(value)
    except ValueError as exc:
        raise ConfigError(str(exc)) from exc


def _validate_timezone(value):
    try:
        return validate_timezone(value)
    except ValueError as exc:
        raise ConfigError(str(exc)) from exc


def _find_member(roles, member_id):
    for item in roles["members"]:
        if item["id"] == member_id:
            return item
    raise NotFoundError(f"Unknown member: {member_id}")


def _find_folder(folders, folder_id):
    for item in folders["folders"]:
        if item["id"] == folder_id:
            return item
    raise NotFoundError(f"Unknown folder: {folder_id}")


def _find_pending(pending, device_id):
    for item in pending["devices"]:
        if item["device_id"] == device_id:
            return item
    raise NotFoundError(f"No pending device: {device_id}")


def _find_accepted(devices, device_id):
    for item in devices["devices"]:
        if item["device_id"] == device_id:
            return item
    raise NotFoundError(f"Device has not been accepted: {device_id}")


def _reject_forbidden_keys(extra):
    for key in extra:
        normalised = str(key).strip().lower().replace("-", "_")
        if normalised in FORBIDDEN_PROFILE_KEYS:
            raise ConfigError("Government ID is not stored.")
        if normalised not in ALLOWED_ORG_KEYS:
            raise ConfigError(f"Unknown organisation field: {key}")


def _permission_message(role, action):
    if role == "member" and action == "device.accept":
        return MEMBER_CANNOT_ACCEPT
    if action == "device.accept":
        return MEMBER_CANNOT_ACCEPT
    if action == "folder.share":
        return f"Role {role} cannot share or unshare a folder."
    if action == "folder.write":
        return f"Role {role} cannot change folders or access notes."
    if action == "role.assign":
        return "Only the owner can assign roles."
    if action == "org.set_name":
        return "Only the owner can change the organisation name."
    if action == "org.set_profile":
        return f"Role {role} cannot change the organisation profile."
    if action == "audit.export":
        return f"Role {role} cannot export the audit log."
    return f"Role {role} cannot perform {action}."


# Re-export audit columns for tests that check CSV shape.
AUDIT_CSV_COLUMNS = AUDIT_COLUMNS
