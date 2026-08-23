"""Local roles for a BlakSync organisation.

A member can sync accepted folders and cannot accept a new device.
There is no social login. Roles are stored only in the local config directory.
"""

ROLES = ("owner", "admin", "member")

# owner always retains pairing rights; member is never allowed to pair.
PAIR_ROLES = ("owner", "admin")

PERMISSIONS = {
    "org.set_name": ("owner",),
    "org.set_profile": ("owner", "admin"),
    "role.assign": ("owner",),
    "folder.write": ("owner", "admin"),
    "device.accept": ("owner", "admin"),
    "device.deny": ("owner", "admin"),
    "device.pending_record": ("owner", "admin"),
    "folder.share": ("owner", "admin"),
    "audit.export": ("owner", "admin"),
}

MEMBER_CANNOT_ACCEPT = "Members cannot accept a new device."


def parse_role(value):
    role = (value or "").strip().lower()
    if role not in ROLES:
        raise ValueError(f"Role must be one of: {', '.join(ROLES)}.")
    return role


def parse_who_may_pair(values):
    """Return the roles allowed to pair a folder. Members cannot be listed."""
    if values is None:
        return list(PAIR_ROLES)
    if isinstance(values, str):
        parts = [item.strip() for item in values.split(",") if item.strip()]
    else:
        parts = [str(item).strip() for item in values if str(item).strip()]
    if not parts:
        return list(PAIR_ROLES)
    roles = []
    for item in parts:
        role = parse_role(item)
        if role == "member":
            raise ValueError(
                "Members cannot pair a new device; who-may-pair may only include owner and admin."
            )
        if role not in roles:
            roles.append(role)
    return roles


def can(role, action):
    allowed = PERMISSIONS.get(action)
    if not allowed:
        return False
    return role in allowed
