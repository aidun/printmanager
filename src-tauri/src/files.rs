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

/// Kennung eines Auftrags im Dateinamen, z.B. `A0004`. Bambu Studio übernimmt den Dateinamen als Projekt-
/// und Druckauftragsnamen; so lässt sich ein laufender Druck eindeutig dem Auftrag zuordnen.
pub fn order_tag(number: i64) -> String {
    format!("A{number:04}")
}

pub fn is_model(name: &str) -> bool {
    let n = name.to_lowercase();
    [".3mf", ".stl", ".step", ".stp", ".obj"].iter().any(|e| n.ends_with(e))
}

/// Modelldateien bekommen die Auftragskennung vorangestellt (`A0004 benchy.stl`), andere Dateien bleiben.
pub fn tagged_name(number: i64, name: &str) -> String {
    let name = safe_name(name);
    let tag = order_tag(number);
    if !is_model(&name) || name.starts_with(&format!("{tag} ")) || name.starts_with(&format!("{tag}_")) {
        return name;
    }
    format!("{tag} {name}")
}

pub fn is_image(name: &str) -> bool {
    mime_guess::from_path(name).first().map(|m| m.type_() == "image").unwrap_or(false)
}

/// Datei in den Auftragsordner kopieren; liefert den neuen Dateinamen.
pub fn copy_into(dir: &Path, src: &Path) -> Result<String> {
    copy_into_as(dir, src, src.file_name().and_then(|n| n.to_str()).unwrap_or("datei"))
}

/// Datei unter einem gewünschten Namen in den Ordner kopieren (frei gemacht mit „(2)“ usw.).
pub fn copy_into_as(dir: &Path, src: &Path, name: &str) -> Result<String> {
    if !src.is_file() {
        return Err(AppError::Invalid(format!("Keine Datei: {}", src.display())));
    }
    std::fs::create_dir_all(dir)?;
    let name = unique_name(dir, name);
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
        assert_eq!(tagged_name(4, "3dbenchy.stl"), "A0004 3dbenchy.stl");
        assert_eq!(tagged_name(4, "A0004 3dbenchy.stl"), "A0004 3dbenchy.stl");
        assert_eq!(tagged_name(12, "bild.jpg"), "bild.jpg");
        assert_eq!(tagged_name(12345, "x.3MF"), "A12345 x.3MF");
    }
}
