//! 3MF-Dateien (Bambu Studio / MakerWorld) lesen: Titel, Beschreibung, Vorschau, Filamente,
//! bei gesliced gespeicherten Dateien auch Druckzeit und Gewicht. Portiert aus `poc/threemf.js`.

use crate::error::{AppError, Result};
use regex::Regex;
use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Filament {
    #[serde(rename = "type")]
    pub kind: String,
    pub color: String,
    /// verbrauchte Gramm (nur bei gesliced Dateien)
    pub grams: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub title: String,
    pub description: String,
    pub designer: String,
    pub license: String,
    #[serde(skip)]
    pub thumbnail: Option<Vec<u8>>,
    pub plates: i64,
    pub printer: String,
    pub print_seconds: Option<i64>,
    pub weight_grams: Option<f64>,
    pub filaments: Vec<Filament>,
    pub sliced: bool,
}

static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static RE_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>|</(p|div|li|h\d)>").unwrap());
static RE_BOOST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<boostme>.*?</boostme>").unwrap());
static RE_ENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z]+);").unwrap());
static RE_PLATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<plate>.*?</plate>").unwrap());
static RE_FIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<filament\s[^>]*>").unwrap());
static RE_ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(\w+)="([^"]*)""#).unwrap());
static RE_META: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="(prediction|weight)" value="([^"]*)""#).unwrap());
static RE_EXTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="extruder" value="(\d+)""#).unwrap());
static RE_PLATE_PNG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Metadata/plate_\d+\.png$").unwrap());

pub fn decode_entities(s: &str) -> String {
    RE_ENT
        .replace_all(s, |c: &regex::Captures| {
            let e = &c[1];
            let r = if let Some(h) = e.strip_prefix("#x") {
                u32::from_str_radix(h, 16).ok().and_then(char::from_u32).map(String::from)
            } else if let Some(d) = e.strip_prefix('#') {
                d.parse().ok().and_then(char::from_u32).map(String::from)
            } else {
                match e.to_lowercase().as_str() {
                    "lt" => Some("<".into()),
                    "gt" => Some(">".into()),
                    "quot" => Some("\"".into()),
                    "apos" => Some("'".into()),
                    "amp" => Some("&".into()),
                    "nbsp" => Some(" ".into()),
                    _ => None,
                }
            };
            r.unwrap_or_else(|| c[0].to_string())
        })
        .into_owned()
}

/// HTML (bei MakerWorld oft doppelt kodiert) in schlichten Text umwandeln.
pub fn html_to_text(html: &str) -> String {
    let mut s = html.to_string();
    for _ in 0..3 {
        if !s.contains("&lt;") && !s.contains("&amp;") && !s.contains("&quot;") {
            break;
        }
        s = decode_entities(&s);
    }
    let s = RE_BOOST.replace_all(&s, "");
    let s = RE_BR.replace_all(&s, "\n");
    let s = RE_TAG.replace_all(&s, "");
    let s = decode_entities(&s);
    let lines: Vec<&str> = s.lines().map(str::trim).collect();
    let mut out = lines.join("\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

fn short_color(c: &str) -> String {
    let ok = c.len() >= 7 && c.starts_with('#') && c[1..7].chars().all(|x| x.is_ascii_hexdigit());
    if ok { c[..7].to_uppercase() } else { String::new() }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub fn analyze(path: &Path) -> Result<Info> {
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| AppError::Invalid("Keine gültige 3MF-Datei".into()))?;
    let names: Vec<String> = zip.file_names().map(String::from).collect();
    if !names.iter().any(|n| n.to_lowercase().ends_with(".model")) {
        return Err(AppError::Invalid("Keine gültige 3MF-Datei (Modell fehlt)".into()));
    }
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut f = zip.by_name(name).ok()?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).ok()?;
        Some(buf)
    };
    let text = |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();

    let mut info = Info::default();

    // Metadaten im Hauptmodell (MakerWorld-Downloads bzw. Projekt-Tab in Bambu Studio)
    let model = text(read("3D/3dmodel.model"));
    let mut end = model.len().min(300_000);
    while !model.is_char_boundary(end) {
        end -= 1;
    }
    let model = &model[..end];
    let meta = |key: &str| -> String {
        let open = format!("<metadata name=\"{key}\">");
        model
            .find(&open)
            .and_then(|i| {
                let rest = &model[i + open.len()..];
                rest.find("</metadata>").map(|j| rest[..j].to_string())
            })
            .unwrap_or_default()
    };
    info.title = truncate(decode_entities(&meta("Title")).trim(), 120);
    info.description = truncate(&html_to_text(&meta("Description")), 5000);
    info.designer = truncate(decode_entities(&meta("Designer")).trim(), 80);
    info.license = truncate(decode_entities(&meta("License")).trim(), 40);

    // Vorschaubild
    let mut plate_imgs: Vec<&String> = names.iter().filter(|n| RE_PLATE_PNG.is_match(n)).collect();
    plate_imgs.sort();
    info.plates = plate_imgs.len().max(1) as i64;
    let mut candidates = vec!["Metadata/plate_1.png".to_string()];
    candidates.extend(plate_imgs.iter().map(|s| s.to_string()));
    candidates.extend(["Metadata/thumbnail.png".into(), "Metadata/top_1.png".into()]);
    for c in candidates {
        if names.contains(&c) {
            if let Some(b) = read(&c).filter(|b| !b.is_empty()) {
                info.thumbnail = Some(b);
                break;
            }
        }
    }

    // Projekteinstellungen
    let settings: serde_json::Value =
        serde_json::from_str(&text(read("Metadata/project_settings.config"))).unwrap_or_default();
    info.printer = settings["printer_model"].as_str().unwrap_or("").to_string();

    // Gesliced: Druckzeit, Gewicht, Filamente aus slice_info.config
    let slice = text(read("Metadata/slice_info.config"));
    let (mut secs, mut grams) = (0f64, 0f64);
    for pl in RE_PLATE.find_iter(&slice) {
        let pl = pl.as_str();
        // je Platte zählt der erste Wert eines Schlüssels
        let (mut p, mut w) = (None, None);
        for c in RE_META.captures_iter(pl) {
            let x = c[2].parse::<f64>().ok();
            match &c[1] {
                "prediction" if p.is_none() => p = x,
                "weight" if w.is_none() => w = x,
                _ => {}
            }
        }
        secs += p.unwrap_or(0.0);
        grams += w.unwrap_or(0.0);
        for tag in RE_FIL.find_iter(pl) {
            let mut kind = String::new();
            let mut color = String::new();
            let mut used = None;
            for a in RE_ATTR.captures_iter(tag.as_str()) {
                match &a[1] {
                    "type" => kind = decode_entities(&a[2]),
                    "color" => color = short_color(&a[2]),
                    "used_g" => used = a[2].parse::<f64>().ok(),
                    _ => {}
                }
            }
            match info.filaments.iter_mut().find(|f| f.kind == kind && f.color == color) {
                Some(f) => f.grams = Some(f.grams.unwrap_or(0.0) + used.unwrap_or(0.0)),
                None => info.filaments.push(Filament { kind, color, grams: used }),
            }
        }
    }
    if secs > 0.0 {
        info.print_seconds = Some(secs.round() as i64);
        info.sliced = true;
    }
    if grams > 0.0 {
        info.weight_grams = Some((grams * 10.0).round() / 10.0);
    }

    // Nicht gesliced: benutzte Filamente aus der Objekt-Zuordnung
    if info.filaments.is_empty() {
        if let Some(colors) = settings["filament_colour"].as_array() {
            let ms = text(read("Metadata/model_settings.config"));
            let mut used: Vec<usize> = RE_EXTR.captures_iter(&ms).filter_map(|c| c[1].parse().ok()).collect();
            used.sort();
            used.dedup();
            if used.is_empty() && !colors.is_empty() {
                used.push(1);
            }
            for i in used {
                let color = colors.get(i - 1).and_then(|c| c.as_str()).map(short_color).unwrap_or_default();
                let kind = settings["filament_type"][i - 1].as_str().unwrap_or("").to_string();
                if !color.is_empty() {
                    info.filaments.push(Filament { kind, color, grams: None });
                }
            }
        }
    }
    info.filaments.truncate(16);
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_und_entities() {
        assert_eq!(html_to_text("&lt;p&gt;Hallo&amp;nbsp;Welt&lt;/p&gt;&lt;br&gt;Zeile 2"), "Hallo Welt\n\nZeile 2");
        assert_eq!(decode_entities("&#228;&#x00FC;&amp;"), "äü&");
        assert_eq!(short_color("#bb3d43ff"), "#BB3D43");
        assert_eq!(short_color("rot"), "");
    }

    /// Echte Dateien aus `tests/fixtures/`, falls vorhanden (nur lokal, nicht im Repo).
    #[test]
    fn echte_dateien() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let switch = base.join("switch.3mf");
        if switch.exists() {
            let i = analyze(&switch).unwrap();
            assert!(!i.title.is_empty());
            assert!(i.thumbnail.is_some());
            assert!(!i.filaments.is_empty());
        }
        let sliced = base.join("figuren.gcode.3mf");
        if sliced.exists() {
            let i = analyze(&sliced).unwrap();
            assert!(i.sliced && i.print_seconds.unwrap() > 0);
            assert!(i.filaments[0].grams.unwrap() > 0.0);
        }
    }
}
