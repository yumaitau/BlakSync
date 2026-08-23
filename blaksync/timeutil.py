"""Australia/* timezone helpers for the org profile and audit timestamps."""

from datetime import datetime
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError


def validate_timezone(value):
    name = (value or "").strip()
    if not name.startswith("Australia/"):
        raise ValueError("Timezone must be an IANA name under Australia/ (for example Australia/Darwin).")
    try:
        ZoneInfo(name)
    except ZoneInfoNotFoundError as exc:
        raise ValueError(f"Unknown timezone: {name}") from exc
    return name


def stamp(timezone):
    """Return an ISO-8601 timestamp in the organisation's timezone."""
    return datetime.now(ZoneInfo(timezone)).isoformat(timespec="seconds")
