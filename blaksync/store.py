"""Config-directory store. Each directory is one organisation and cannot see another."""

from __future__ import annotations

import json
import os
from contextlib import contextmanager
from pathlib import Path

from .errors import ConfigError

ORG_FILE = "org.json"
ROLES_FILE = "roles.json"
FOLDERS_FILE = "folders.json"
PENDING_FILE = "pending-devices.json"
DEVICES_FILE = "devices.json"
AUDIT_FILE = "audit.jsonl"
LOCK_FILE = ".lock"

EMPTY_ROLES = {"local_member_id": None, "members": []}
EMPTY_FOLDERS = {"folders": []}
EMPTY_PENDING = {"devices": []}
EMPTY_DEVICES = {"devices": []}


class ConfigStore:
    """JSON/JSONL files inside one config directory."""

    def __init__(self, config_dir):
        if not config_dir:
            raise ConfigError("A config directory is required.")
        self.config_dir = Path(config_dir).expanduser().resolve()

    def ensure_dir(self):
        self.config_dir.mkdir(parents=True, exist_ok=True)
        try:
            os.chmod(self.config_dir, 0o700)
        except OSError:
            pass

    @contextmanager
    def lock(self):
        self.ensure_dir()
        lock_path = self.config_dir / LOCK_FILE
        with open(lock_path, "a+", encoding="utf-8") as handle:
            _exclusive_lock(handle)
            try:
                yield
            finally:
                _unlock(handle)

    def org_exists(self):
        return (self.config_dir / ORG_FILE).is_file()

    def read_org(self):
        return self._read_json(ORG_FILE, default=None)

    def write_org(self, data):
        self._write_json(
            ORG_FILE,
            {
                "name": data["name"],
                "timezone": data["timezone"],
                "contact": data["contact"],
            },
        )

    def read_roles(self):
        return self._read_json(ROLES_FILE, default=dict(EMPTY_ROLES))

    def write_roles(self, data):
        self._write_json(ROLES_FILE, data)

    def read_folders(self):
        return self._read_json(FOLDERS_FILE, default=dict(EMPTY_FOLDERS))

    def write_folders(self, data):
        self._write_json(FOLDERS_FILE, data)

    def read_pending(self):
        return self._read_json(PENDING_FILE, default=dict(EMPTY_PENDING))

    def write_pending(self, data):
        self._write_json(PENDING_FILE, data)

    def read_devices(self):
        return self._read_json(DEVICES_FILE, default=dict(EMPTY_DEVICES))

    def write_devices(self, data):
        self._write_json(DEVICES_FILE, data)

    def append_audit(self, row):
        self.ensure_dir()
        path = self.config_dir / AUDIT_FILE
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")

    def read_audit(self):
        path = self.config_dir / AUDIT_FILE
        if not path.is_file():
            return []
        rows = []
        with open(path, encoding="utf-8") as handle:
            for line in handle:
                line = line.strip()
                if not line:
                    continue
                rows.append(json.loads(line))
        return rows

    def _read_json(self, name, default):
        path = self.config_dir / name
        if not path.is_file():
            if default is None:
                return None
            return json.loads(json.dumps(default))
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)

    def _write_json(self, name, data):
        self.ensure_dir()
        path = self.config_dir / name
        tmp = path.with_name(path.name + ".tmp")
        text = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
        tmp.write_text(text, encoding="utf-8")
        tmp.replace(path)


def _exclusive_lock(handle):
    try:
        import fcntl

        fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
    except ImportError:
        pass


def _unlock(handle):
    try:
        import fcntl

        fcntl.flock(handle.fileno(), fcntl.LOCK_UN)
    except ImportError:
        pass
