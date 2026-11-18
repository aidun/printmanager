//! Gemeinsame Helfer für Spulen aus Slicern und Konten.
//! Formate: `spools.json` von Bambu Studio in `slicers/bambu_studio.rs`, Bambu-Konto in `bambu/spools.rs`.

/// Farbe als `#RRGGBB` (Großbuchstaben); Alpha wird abgeschnitten, ungültige Werte werden leer.
pub fn color(c: &str) -> String {
    let ok = c.len() >= 7 && c.starts_with('#') && c[1..7].chars().all(|x| x.is_ascii_hexdigit());
    if ok { c[..7].to_uppercase() } else { String::new() }
}
