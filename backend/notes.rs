use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{ConfigStore, default_config_dir};
use crate::{Error, Result};

const NOTES_FILE: &str = "access-notes.json";

#[derive(Clone, Debug)]
pub struct AccessNotesStore {
    store: ConfigStore,
}

#[derive(Default, Deserialize, Serialize)]
struct NotesFile {
    folders: BTreeMap<String, String>,
}

impl AccessNotesStore {
    pub fn new(config_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: ConfigStore::new(config_dir)?,
        })
    }

    pub fn from_default() -> Result<Self> {
        Self::new(default_config_dir())
    }

    pub fn file_path(&self) -> PathBuf {
        self.store.path(NOTES_FILE)
    }

    pub fn read_all(&self) -> Result<BTreeMap<String, String>> {
        Ok(self
            .store
            .read_json(NOTES_FILE, NotesFile::default())?
            .folders)
    }

    pub fn get(&self, folder_id: &str) -> Result<String> {
        Ok(self.read_all()?.remove(folder_id).unwrap_or_default())
    }

    pub fn set(&self, folder_id: &str, note: &str) -> Result<String> {
        if folder_id.is_empty() {
            return Err(Error::Config("Folder ID is required".into()));
        }
        self.store.with_lock(|| {
            let mut file = self.store.read_json(NOTES_FILE, NotesFile::default())?;
            let note = note.trim().to_string();
            if note.is_empty() {
                file.folders.remove(folder_id);
            } else {
                file.folders.insert(folder_id.to_string(), note.clone());
            }
            self.store.write_json(NOTES_FILE, &file)?;
            Ok(note)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_clears_note() {
        let temporary = tempfile::tempdir().unwrap();
        let notes = AccessNotesStore::new(temporary.path()).unwrap();
        assert_eq!(notes.get("heritage").unwrap(), "");
        notes
            .set("heritage", " Speak with the cultural officer. ")
            .unwrap();
        assert_eq!(
            notes.get("heritage").unwrap(),
            "Speak with the cultural officer."
        );
        notes.set("heritage", "").unwrap();
        assert_eq!(notes.get("heritage").unwrap(), "");
    }
}
