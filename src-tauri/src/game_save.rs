//! Content-based snapshots shared by monitoring, local backups and cloud uploads.
use crate::config::CompressionLevel;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot(pub BTreeMap<String, String>);

fn digest_reader(mut reader: impl Read) -> Result<String, String> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl Snapshot {
    pub fn fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        for (path, content) in &self.0 {
            hash.update((path.len() as u64).to_le_bytes());
            hash.update(path.as_bytes());
            hash.update(content.as_bytes());
        }
        format!("{:x}", hash.finalize())
    }
}

pub fn scan(root: &Path) -> Result<Snapshot, String> {
    if !root.is_dir() {
        return Err(format!(
            "Save priečinok nie je dostupný: {}",
            root.display()
        ));
    }
    let mut files = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1) {
        let entry = entry.map_err(|e| format!("Kontrola save súborov zlyhala: {e}"))?;
        // Match the archive policy; directory links must never escape the save root.
        if entry.file_type().is_file() {
            let relative = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let file = std::fs::File::open(entry.path())
                .map_err(|e| format!("Nedá sa čítať {}: {e}", entry.path().display()))?;
            files.insert(relative, digest_reader(file)?);
        }
    }
    Ok(Snapshot(files))
}

pub fn scan_archive(path: &Path) -> Result<Snapshot, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_file() {
            let name = entry.name().replace('\\', "/");
            if files.insert(name, digest_reader(entry)?).is_some() {
                return Err("ZIP obsahuje duplicitné save súbory.".into());
            }
        }
    }
    Ok(Snapshot(files))
}

/// Publish a ZIP only if its bytes and the source tree match the expected snapshot.
pub fn create_backup(
    root: &Path,
    backup_dir: &Path,
    expected: &Snapshot,
    level: CompressionLevel,
) -> Result<PathBuf, String> {
    if crate::sync::comparable_path(backup_dir).starts_with(crate::sync::comparable_path(root)) {
        return Err("ZIP zálohy nemôžu byť uložené vo vnútri save priečinka.".into());
    }
    std::fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
    if crate::sync::comparable_path(backup_dir).starts_with(crate::sync::comparable_path(root)) {
        return Err("ZIP zálohy nemôžu byť uložené vo vnútri save priečinka.".into());
    }
    let stem = format!(
        "save_{}_{}",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f"),
        uuid::Uuid::new_v4()
    );
    let partial = backup_dir.join(format!("{stem}.partial"));
    let complete = backup_dir.join(format!("{stem}.zip"));
    let result = (|| {
        crate::archive::zip_dir_to(root, &partial, level)?;
        if &scan_archive(&partial)? != expected || &scan(root)? != expected {
            return Err(
                "Hra počas zálohovania zmenila save. Záloha sa zopakuje po dokončení zápisu."
                    .into(),
            );
        }
        std::fs::rename(&partial, &complete).map_err(|e| e.to_string())?;
        Ok(complete)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(partial);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("backup-sync-snapshot-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(root.join("Saves/Farm1")).unwrap();
            std::fs::create_dir_all(root.join("Saves/Farm2/nested")).unwrap();
            std::fs::write(root.join("Saves/Farm1/save"), b"farm-one").unwrap();
            std::fs::write(root.join("Saves/Farm2/nested/save"), b"old-save").unwrap();
            Self(root)
        }
        fn saves(&self) -> PathBuf {
            self.0.join("Saves")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn detects_nested_same_size_edit_even_with_preserved_timestamp() {
        let fixture = Fixture::new();
        let before = scan(&fixture.saves()).unwrap();
        let file = fixture.saves().join("Farm2/nested/save");
        let modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        std::fs::write(&file, b"new-save").unwrap();
        std::fs::File::options()
            .write(true)
            .open(file)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_ne!(
            before.fingerprint(),
            scan(&fixture.saves()).unwrap().fingerprint()
        );
    }
    #[test]
    fn detects_new_farm_rename_and_deletion() {
        let fixture = Fixture::new();
        let initial = scan(&fixture.saves()).unwrap();
        std::fs::create_dir_all(fixture.saves().join("Farm3")).unwrap();
        std::fs::write(fixture.saves().join("Farm3/save"), b"new-farm").unwrap();
        let added = scan(&fixture.saves()).unwrap();
        assert_ne!(initial, added);
        std::fs::rename(
            fixture.saves().join("Farm3/save"),
            fixture.saves().join("Farm3/renamed"),
        )
        .unwrap();
        assert_ne!(added, scan(&fixture.saves()).unwrap());
        std::fs::remove_file(fixture.saves().join("Farm3/renamed")).unwrap();
        assert_eq!(initial, scan(&fixture.saves()).unwrap());
    }
    #[test]
    fn backup_contains_all_farms_and_stays_after_source_changes() {
        let fixture = Fixture::new();
        let expected = scan(&fixture.saves()).unwrap();
        let zip = create_backup(
            &fixture.saves(),
            &fixture.0.join("backups"),
            &expected,
            CompressionLevel::Fast,
        )
        .unwrap();
        assert_eq!(scan_archive(&zip).unwrap(), expected);
        std::fs::write(fixture.saves().join("Farm1/save"), b"later-save").unwrap();
        assert_eq!(scan_archive(&zip).unwrap(), expected);
    }
    #[test]
    fn rejects_changed_source_without_publishing_partial_backup() {
        let fixture = Fixture::new();
        let expected = scan(&fixture.saves()).unwrap();
        std::fs::write(fixture.saves().join("Farm1/save"), b"changed").unwrap();
        let backups = fixture.0.join("backups");
        assert!(create_backup(
            &fixture.saves(),
            &backups,
            &expected,
            CompressionLevel::Fast
        )
        .is_err());
        assert_eq!(std::fs::read_dir(backups).unwrap().count(), 0);
    }
    #[test]
    fn rejects_backup_inside_save_and_missing_save_root() {
        let fixture = Fixture::new();
        let expected = scan(&fixture.saves()).unwrap();
        assert!(create_backup(
            &fixture.saves(),
            &fixture.saves().join("backup"),
            &expected,
            CompressionLevel::Fast
        )
        .is_err());
        assert!(scan(&fixture.0.join("missing")).is_err());
    }
}
