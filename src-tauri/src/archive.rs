use crate::config::CompressionLevel;
use std::fs::File;
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Zabalí celý priečinok do jedného ZIP archívu na zvolenú cestu.
/// Obsah priečinka uloží priamo do koreňa ZIPu, bez ďalšieho vnoreného priečinka.
pub fn zip_dir_to(src: &Path, out_path: &Path, level: CompressionLevel) -> Result<(), String> {
    if !src.is_dir() {
        return Err(format!("Priečinok neexistuje: {}", src.display()));
    }
    if out_path.exists() {
        return Err(format!("ZIP záloha už existuje: {}", out_path.display()));
    }
    if out_path.starts_with(src) {
        return Err("ZIP záloha nesmie byť uložená vo vnútri zálohovaného priečinka.".into());
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(out_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);

    let deflate_level = match level {
        CompressionLevel::Fast => 1,
        CompressionLevel::Best => 9,
    };
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(deflate_level))
        .large_file(true);

    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.map_err(|e| format!("Čítanie save priečinka zlyhalo: {e}"))?;
        let path = entry.path();
        let file_type = entry.file_type();
        let rel = path
            .strip_prefix(src)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if rel.is_empty() {
            continue;
        }
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            zip.add_directory(format!("{rel}/"), options)
                .map_err(|e| e.to_string())?;
        } else if file_type.is_file() {
            zip.start_file(rel, options).map_err(|e| e.to_string())?;
            let mut f = File::open(path).map_err(|e| e.to_string())?;
            std::io::copy(&mut f, &mut zip).map_err(|e| e.to_string())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

/// Rozbalí zip archív do cieľového priečinka (súbory prepíše, nič nemaže).
pub fn unzip_to(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = ZipArchive::new(file).map_err(|e| e.to_string())?;
    const MAX_ENTRIES: usize = 100_000;
    const MAX_UNCOMPRESSED_BYTES: u64 = 100 * 1024 * 1024 * 1024;
    if zip.len() > MAX_ENTRIES {
        return Err(format!(
            "ZIP obsahuje príliš veľa položiek: {} (maximum {MAX_ENTRIES}).",
            zip.len()
        ));
    }
    let mut total = 0u64;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?;
        if entry.enclosed_name().is_none() {
            return Err(format!("ZIP obsahuje nebezpečnú cestu: {name}"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or_else(|| "Veľkosť ZIP archívu pretiekla.".to_string())?;
        if total > MAX_UNCOMPRESSED_BYTES {
            return Err("Rozbalená záloha by prekročila bezpečnostný limit 100 GB.".into());
        }
    }
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    zip.extract(dest).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "backup-sync-archive-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn zip_round_trip_preserves_files() {
        let source = temp_dir("source");
        let destination = temp_dir("destination");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::write(source.join("nested").join("save.dat"), b"save-data").unwrap();

        let archive = source.with_extension("zip");
        zip_dir_to(&source, &archive, CompressionLevel::Fast).unwrap();
        unzip_to(&archive, &destination).unwrap();

        assert_eq!(
            std::fs::read(destination.join("nested").join("save.dat")).unwrap(),
            b"save-data"
        );

        let _ = std::fs::remove_file(archive);
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(destination);
    }

    #[test]
    fn zip_keeps_save_files_at_destination_root() {
        let source = temp_dir("source-root");
        let destination = temp_dir("destination-root");
        let archive = temp_dir("archive-root").with_extension("zip");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::write(source.join("profile.dat"), b"root-save").unwrap();
        std::fs::write(source.join("nested").join("slot.dat"), b"nested-save").unwrap();

        zip_dir_to(&source, &archive, CompressionLevel::Fast).unwrap();
        unzip_to(&archive, &destination).unwrap();

        assert_eq!(
            std::fs::read(destination.join("profile.dat")).unwrap(),
            b"root-save"
        );
        assert_eq!(
            std::fs::read(destination.join("nested").join("slot.dat")).unwrap(),
            b"nested-save"
        );
        assert!(!destination.join("source-root").exists());

        let _ = std::fs::remove_file(archive);
        let _ = std::fs::remove_dir_all(source);
        let _ = std::fs::remove_dir_all(destination);
    }
}
