"""Pending-device prompt shown before Accept.

The organisation writes each access note. BlakSync does not invent ceremony rules.
Filesystem paths are not shown.
"""

from dataclasses import dataclass, field


@dataclass
class FolderView:
    id: str
    label: str
    org_name: str
    access_note: str
    who_may_pair: list[str] = field(default_factory=list)


@dataclass
class PendingPrompt:
    device_id: str
    device_name: str
    organisation: str
    actor_id: str
    actor_role: str
    can_accept: bool
    refusal_reason: str | None
    folders: list[FolderView]
    text: str

    def to_dict(self):
        return {
            "device_id": self.device_id,
            "device_name": self.device_name,
            "organisation": self.organisation,
            "actor_id": self.actor_id,
            "actor_role": self.actor_role,
            "can_accept": self.can_accept,
            "refusal_reason": self.refusal_reason,
            "folders": [
                {
                    "id": folder.id,
                    "label": folder.label,
                    "org_name": folder.org_name,
                    "access_note": folder.access_note,
                    "who_may_pair": list(folder.who_may_pair),
                }
                for folder in self.folders
            ],
            "text": self.text,
        }


def render_pending_prompt(*, org_name, device_id, device_name, actor_role, folders):
    lines = [
        "Pending device",
        "--------------",
        f"Organisation: {org_name}",
        f"Device name: {device_name}",
        f"Device ID: {device_id}",
        "",
        "Read the access notes before you accept this device.",
        "The organisation writes these notes. BlakSync does not invent ceremony rules.",
        "",
    ]
    if folders:
        lines.append("Folders:")
        for folder in folders:
            lines.append(f"  {folder.label}")
            who = ", ".join(folder.who_may_pair) if folder.who_may_pair else "owner, admin"
            lines.append(f"    Who may pair: {who}")
            note = folder.access_note.strip() if folder.access_note else "(No access note has been written yet.)"
            lines.append(f"    Access note: {note}")
            lines.append("")
    else:
        lines.append("No folders are listed for this pairing yet.")
        lines.append("")

    lines.append(f"Your role: {actor_role}")
    if actor_role == "member":
        lines.append("You cannot accept this device. A member cannot accept a new device.")
    else:
        lines.append(
            "Accepting this device lets it connect. Share folders separately after you accept."
        )
    return "\n".join(lines).rstrip() + "\n"
