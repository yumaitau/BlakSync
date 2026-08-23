"""Local audit log and CSV export.

Records device accepted, folder shared, and folder unshared. Folder labels
are included. Filesystem paths and file contents are not.
"""

import csv
import io

from .errors import ConfigError

EVENTS = ("device_accepted", "folder_shared", "folder_unshared")
COLUMNS = ("timestamp", "event", "actor", "role", "device_id", "folder_label")


def audit_row(*, timestamp, event, actor, role, device_id="", folder_label=""):
    if event not in EVENTS:
        raise ConfigError(f"Unknown audit event: {event}")
    return {
        "timestamp": timestamp,
        "event": event,
        "actor": actor,
        "role": role,
        "device_id": device_id or "",
        "folder_label": folder_label or "",
    }


def to_csv(rows):
    buffer = io.StringIO()
    writer = csv.DictWriter(
        buffer,
        fieldnames=list(COLUMNS),
        extrasaction="ignore",
        lineterminator="\n",
    )
    writer.writeheader()
    for row in rows:
        writer.writerow({column: row.get(column, "") for column in COLUMNS})
    return buffer.getvalue()
