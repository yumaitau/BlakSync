use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Role(String),
    #[error("{message}")]
    Syncthing { message: String, status: u16 },
    #[error("{0}")]
    Process(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Csv(#[from] csv::Error),
}

impl Error {
    pub fn syncthing_status(&self) -> Option<u16> {
        match self {
            Self::Syncthing { status, .. } => Some(*status),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
