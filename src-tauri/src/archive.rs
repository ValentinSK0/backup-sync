use crate::config::CompressionLevel;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Zabalí celý priečinok do jedného zip archívu v temp priečinku a vráti jeho cestu.
pub fn zip_dir(src: &Path, level: CompressionLevel) -> Result<PathBuf, String> {
    if !src.is_dir() {
        return Err(format!("Priečinok neexistuje: {}", src.display()));
    }
    let out_path = std::env::temp_dir().join(format!("backup-sync-{}.zip", uuid::Uuid::new_v4()));
    let file = File::create(&out_path).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);

    let deflate_level = match level {
        CompressionLevel::Fast => 1,
        CompressionLevel::Best => 9,
    };
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(deflate_level))
        .large_file(true);

    let mut buffer = Vec::new();
    for entry in walkdir::WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path
            .strip_prefix(src)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if rel.is_empty() {
            continue;
        }
        if path.is_dir() {
            zip.add_directory(format!("{rel}/"), options)
                .map_err(|e| e.to_string())?;
        } else if path.is_file() {
            zip.start_file(rel, options).map_err(|e| e.to_string())?;
            let mut f = File::open(path).map_err(|e| e.to_string())?;
            buffer.clear();
            f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            zip.write_all(&buffer).map_err(|e| e.to_string())?;
        }
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(out_path)
}

/// Rozbalí zip archív do cieľového priečinka (súbory prepíše, nič nemaže).
pub fn unzip_to(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = ZipArchive::new(file).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    zip.extract(dest).map_err(|e| e.to_string())
}
