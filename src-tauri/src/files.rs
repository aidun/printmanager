//! Dateien im Datenordner: `orders/<nummer>/…`

use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

pub fn order_dir(data_dir: &Path, number: i64) -> PathBuf {
    data_dir.join("orders").join(format!("{number:04}"))
}

/// Dateiname ohne Pfadanteile und problematische Zeichen.
pub fn safe_name(name: &str) -> String {
    let base = Path::new(name).file_name().and_then(|n| n.to_str()).unwrap_or("datei");
    let cleaned: String = base
        .chars()
        .map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() { "datei".into() } else { cleaned }
}

/// Freien Namen im Ordner finden: `bild.jpg`, `bild (2).jpg`, …
pub fn unique_name(dir: &Path, name: &str) -> String {
    let name = safe_name(name);
    if !dir.join(&name).exists() {
        return name;
    }
    let p = Path::new(&name);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("datei");
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
    (2..)
        .map(|i| format!("{stem} ({i}){ext}"))
        .find(|n| !dir.join(n).exists())
        .unwrap()
}

pub fn is_image(name: &str) -> bool {
    mime_guess::from_path(name).first().map(|m| m.type_() == "image").unwrap_or(false)
}

/// Datei in den Auftragsordner kopieren; liefert den neuen Dateinamen.
pub fn copy_into(dir: &Path, src: &Path) -> Result<String> {
    if !src.is_file() {
        return Err(AppError::Invalid(format!("Keine Datei: {}", src.display())));
    }
    std::fs::create_dir_all(dir)?;
    let name = unique_name(dir, src.file_name().and_then(|n| n.to_str()).unwrap_or("datei"));
    std::fs::copy(src, dir.join(&name))?;
    Ok(name)
}

pub fn write_into(dir: &Path, name: &str, bytes: &[u8]) -> Result<String> {
    std::fs::create_dir_all(dir)?;
    let name = unique_name(dir, name);
    std::fs::write(dir.join(&name), bytes)?;
    Ok(name)
}

/// Pfad eines Anhangs auflösen und sicherstellen, dass er im Auftragsordner liegt.
pub fn resolve_in(dir: &Path, rel: &str) -> Result<PathBuf> {
    let p = dir.join(safe_name(rel));
    if p.parent() != Some(dir) {
        return Err(AppError::Invalid("Ungültiger Pfad".into()));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namen() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("a:b?.jpg"), "a_b_.jpg");
        assert_eq!(safe_name(".."), "datei");
        let d = tempfile::tempdir().unwrap();
        assert_eq!(write_into(d.path(), "bild.jpg", b"1").unwrap(), "bild.jpg");
        assert_eq!(write_into(d.path(), "bild.jpg", b"2").unwrap(), "bild (2).jpg");
        assert!(is_image("x.PNG") && !is_image("x.3mf"));
    }
}
