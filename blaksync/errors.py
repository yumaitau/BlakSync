"""Error types for the BlakSync org overlay."""


class BlakSyncError(Exception):
    """Base error for the org overlay."""


class ConfigError(BlakSyncError):
    """Invalid or missing local configuration."""


class NotFoundError(BlakSyncError):
    """Requested org object does not exist in this config directory."""


class RoleError(BlakSyncError):
    """The actor's role cannot perform this action."""

    def __init__(self, message, prompt=None):
        super().__init__(message)
        self.prompt = prompt
