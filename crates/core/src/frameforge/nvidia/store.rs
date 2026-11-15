//! Profile snapshots taken before every driver change, and their index.
//!
//! Each snapshot is an ordinary `.nip` file in `FrameForge\nvidia\backups\`
//! (so it also opens in NVIDIA Profile Inspector) plus one entry in
//! `index.json` that says why it was taken, what was about to change and
//! whether the change was verified. These files are never cleaned up
//! automatically.

use super::nip::{self, NipFile, NipProfile};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const INDEX_SCHEMA: u32 = 1;

/// One line of "what changes".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub profile: String,
    pub setting_id: Option<u32>,
    pub what: String,
    pub before: String,
    pub after: String,
    /// Anything the user should know before agreeing (undocumented setting, restart needed…).
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupReason {
    /// Before an edit in the profile editor.
    Edit,
    Import,
    /// Saved by the user.
    Manual,
    /// Before restoring another backup.
    Restore,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub id: String,
    pub created_at: u64,
    pub label: String,
    pub reason: BackupReason,
    /// Profiles whose state the snapshot holds.
    pub profiles: Vec<String>,
    /// Profiles that did not exist when the snapshot was taken; restoring removes them.
    #[serde(default)]
    pub created_profiles: Vec<String>,
    pub file: String,
    #[serde(default)]
    pub changes: Vec<DiffLine>,
    /// `Some(true)`: the change was read back and matched. `Some(false)`: it did not. `None`: nothing was applied.
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub driver_version: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    schema: u32,
    entries: Vec<BackupEntry>,
}

pub struct BackupStore {
    dir: PathBuf,
}

impl BackupStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn load(&self) -> Index {
        std::fs::read(self.index_path()).ok().and_then(|b| serde_json::from_slice::<Index>(&b).ok()).unwrap_or_default()
    }

    fn store(&self, mut index: Index) -> anyhow::Result<()> {
        index.schema = INDEX_SCHEMA;
        crate::frameforge::write_atomic(&self.index_path(), &serde_json::to_vec_pretty(&index)?)
    }

    /// Newest first.
    pub fn list(&self) -> Vec<BackupEntry> {
        // Entries are appended in order; the sort is stable, so entries from
        // the same second stay newest first.
        let mut e = self.load().entries;
        e.reverse();
        e.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        e
    }

    pub fn get(&self, id: &str) -> Option<BackupEntry> {
        self.load().entries.into_iter().find(|e| e.id == id)
    }

    /// Write a snapshot and its index entry. The file is on disk before this returns.
    pub fn create(&self, label: &str, reason: BackupReason, profiles: Vec<NipProfile>, created_profiles: Vec<String>, changes: Vec<DiffLine>, driver_version: Option<String>) -> anyhow::Result<BackupEntry> {
        let now = crate::frameforge::hardware::now_secs();
        let id = format!("{now}-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let file = format!("{id}.nip");
        let names = profiles.iter().map(|p| p.name.clone()).collect();
        crate::frameforge::write_atomic(&self.dir.join(&file), &nip::write(&NipFile::new(profiles)))?;
        let entry = BackupEntry { id, created_at: now, label: label.chars().take(160).collect(), reason, profiles: names, created_profiles, file, changes, verified: None, note: None, driver_version };
        let mut index = self.load();
        index.entries.push(entry.clone());
        self.store(index)?;
        Ok(entry)
    }

    pub fn update(&self, id: &str, f: impl FnOnce(&mut BackupEntry)) -> anyhow::Result<()> {
        let mut index = self.load();
        if let Some(e) = index.entries.iter_mut().find(|e| e.id == id) {
            f(e);
        }
        self.store(index)
    }

    /// The snapshot's profiles.
    pub fn read(&self, id: &str) -> anyhow::Result<(BackupEntry, NipFile)> {
        let entry = self.get(id).ok_or_else(|| anyhow::anyhow!("That backup no longer exists"))?;
        // The file name comes from our own index, but is still confined to the backups folder.
        let name = std::path::Path::new(&entry.file).file_name().ok_or_else(|| anyhow::anyhow!("The backup entry is damaged"))?;
        let bytes = std::fs::read(self.dir.join(name)).map_err(|e| anyhow::anyhow!("The backup file could not be read: {e}"))?;
        let file = nip::parse(&bytes).map_err(|e| anyhow::anyhow!("The backup file is damaged: {e}"))?;
        Ok((entry, file))
    }

    pub fn path_of(&self, id: &str) -> Option<PathBuf> {
        let entry = self.get(id)?;
        Some(self.dir.join(std::path::Path::new(&entry.file).file_name()?))
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let mut index = self.load();
        let Some(pos) = index.entries.iter().position(|e| e.id == id) else { anyhow::bail!("That backup no longer exists") };
        let entry = index.entries.remove(pos);
        if let Some(name) = std::path::Path::new(&entry.file).file_name() {
            let _ = std::fs::remove_file(self.dir.join(name));
        }
        self.store(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameforge::nvidia::nip::NipSetting;
    use crate::frameforge::nvidia::types::Value;

    #[test]
    fn snapshots_persist_and_survive_a_damaged_index() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().to_path_buf());
        let p = NipProfile { name: "Game".into(), settings: vec![NipSetting::new(1, "x", &Value::Dword(2))], ..Default::default() };
        let e = store.create("Before edit", BackupReason::Edit, vec![p.clone()], vec![], vec![], Some("566.14".into())).unwrap();
        store.update(&e.id, |x| x.verified = Some(true)).unwrap();
        // A second store on the same folder sees it (a restart).
        let again = BackupStore::new(dir.path().to_path_buf());
        let (entry, file) = again.read(&e.id).unwrap();
        assert_eq!(entry.verified, Some(true));
        assert_eq!(file.profiles, vec![p]);
        // The snapshot is a normal .nip file.
        assert!(std::fs::read(again.path_of(&e.id).unwrap()).unwrap().starts_with(&[0xEF, 0xBB, 0xBF]));
        again.delete(&e.id).unwrap();
        assert!(again.list().is_empty());
        assert!(again.read(&e.id).is_err());
        std::fs::write(dir.path().join("index.json"), b"{not json").unwrap();
        assert!(again.list().is_empty());
    }

    #[test]
    fn a_tampered_index_cannot_reach_outside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("backups"));
        let e = store.create("x", BackupReason::Manual, vec![], vec![], vec![], None).unwrap();
        std::fs::write(dir.path().join("secret.nip"), b"<ArrayOfProfile/>").unwrap();
        let mut index: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.path().join("backups/index.json")).unwrap()).unwrap();
        index["entries"][0]["file"] = serde_json::json!("../secret.nip");
        std::fs::write(dir.path().join("backups/index.json"), serde_json::to_vec(&index).unwrap()).unwrap();
        // Only the file name is used, so the entry points at a file that is not there.
        assert!(store.read(&e.id).is_err());
    }
}
