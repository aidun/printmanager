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
    /// Slot im Projekt (1-basiert), falls bekannt
    #[serde(skip)]
    pub slot: Option<i64>,
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
    /// gesliced Platten einzeln (Index wie in Bambu Studio, 1-basiert)
    pub plate_list: Vec<PlateInfo>,
}

/// Eine gesliced Platte: Druckzeit, Gewicht und Filamente je Slot
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlateInfo {
    pub index: i64,
    pub seconds: Option<i64>,
    pub grams: Option<f64>,
    pub filaments: Vec<Filament>,
}

static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static RE_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<br\s*/?>|</(p|div|li|h\d)>").unwrap());
static RE_BOOST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<boostme>.*?</boostme>").unwrap());
static RE_ENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z]+);").unwrap());
static RE_PLATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<plate>.*?</plate>").unwrap());
static RE_FIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<filament\s[^>]*>").unwrap());
static RE_ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(\w+)="([^"]*)""#).unwrap());
static RE_META: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="(prediction|weight)" value="([^"]*)""#).unwrap());
static RE_PLATER_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="plater_id" value="(\d+)""#).unwrap());
static RE_PLATER_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="plater_name" value="([^"]*)""#).unwrap());
static RE_PLATE_IDX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^Metadata/plate_(\d+)\.png$").unwrap());
static RE_INDEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="index" value="(\d+)""#).unwrap());
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
    for (n, pl) in RE_PLATE.find_iter(&slice).enumerate() {
        let pl = pl.as_str();
        let mut plate = PlateInfo {
            index: RE_INDEX.captures(pl).and_then(|c| c[1].parse().ok()).unwrap_or(n as i64 + 1),
            ..Default::default()
        };
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
        plate.seconds = p.map(|x| x.round() as i64);
        plate.grams = w;
        for tag in RE_FIL.find_iter(pl) {
            let mut kind = String::new();
            let mut color = String::new();
            let mut used = None;
            let mut slot = None;
            for a in RE_ATTR.captures_iter(tag.as_str()) {
                match &a[1] {
                    "id" => slot = a[2].parse::<i64>().ok(),
                    "type" => kind = decode_entities(&a[2]),
                    "color" => color = short_color(&a[2]),
                    "used_g" => used = a[2].parse::<f64>().ok(),
                    _ => {}
                }
            }
            // je Slot zusammenfassen (über Platten hinweg); ohne Slot-Nummer nach Material und Farbe.
            // Zwei Slots mit gleichem Filament bleiben getrennt, sie können auf verschiedenen Spulen liegen.
            let same = |f: &&mut Filament| match (slot, f.slot) {
                (Some(a), Some(b)) => a == b,
                _ => f.kind == kind && f.color == color,
            };
            if used.unwrap_or(0.0) > 0.0 {
                plate.filaments.push(Filament { kind: kind.clone(), color: color.clone(), grams: used, slot });
            }
            match info.filaments.iter_mut().find(same) {
                Some(f) => f.grams = Some(f.grams.unwrap_or(0.0) + used.unwrap_or(0.0)),
                None => info.filaments.push(Filament { kind, color, grams: used, slot }),
            }
        }
        if plate.seconds.is_some() || !plate.filaments.is_empty() {
            info.plate_list.push(plate);
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
                    info.filaments.push(Filament { kind, color, grams: None, slot: Some(i as i64) });
                }
            }
        }
    }
    info.filaments.truncate(16);
    Ok(info)
}

/// Filament-Slot eines Projekts (1-basiert)
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelSlot {
    pub slot: i64,
    pub material: String,
    pub color: String,
    /// Name des Filament-Presets im Projekt
    pub name: String,
}

/// Objekt im Projekt mit seinem Slot; `fixed`, wenn Teile des Objekts eigene Slots haben
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelObject {
    pub index: i64,
    pub name: String,
    pub slot: i64,
    pub fixed: bool,
    /// Slots der Teile des Objekts (sortiert, ohne Doppelte)
    pub parts: Vec<i64>,
}

/// Aufbau eines Modells für die Druckvorbereitung
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub slots: Vec<ModelSlot>,
    pub objects: Vec<ModelObject>,
    /// im Modell bemalt (Farben je Fläche); die Zuordnung je Objekt gilt dann nur teilweise
    pub painted: bool,
    /// Platten des Projekts (Index 1-basiert, Name wie in Bambu Studio, oft leer)
    pub plates: Vec<LayoutPlate>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPlate {
    pub index: i64,
    pub name: String,
}

static RE_OBJECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?s)<object id="\d+">(.*?)</object>"#).unwrap());
static RE_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"key="name" value="([^"]*)""#).unwrap());

/// Kopie eines 3MF schreiben, in der Objekte (1-basiert in der Reihenfolge von `model_settings.config`)
/// den angegebenen Slot haben. Teile mit eigenem Slot und bemalte Flächen bleiben, wie sie sind.
/// Die Kommandozeile von Bambu Studio kennt `--load-filament-ids` nur je Datei, nicht je Objekt.
pub fn with_object_slots(src: &Path, dst: &Path, slots: &[(i64, i64)]) -> Result<()> {
    let mut zin = zip::ZipArchive::new(std::fs::File::open(src)?).map_err(|_| AppError::Invalid("Keine gültige 3MF-Datei".into()))?;
    let mut ms = String::new();
    if let Ok(mut f) = zin.by_name("Metadata/model_settings.config") {
        f.read_to_string(&mut ms)?;
    }
    let patched = set_object_slots(&ms, slots);
    let mut out = zip::ZipWriter::new(std::fs::File::create(dst)?);
    for i in 0..zin.len() {
        let f = zin.by_index_raw(i).map_err(|e| AppError::Invalid(format!("3MF nicht lesbar: {e}")))?;
        if f.name() == "Metadata/model_settings.config" {
            continue;
        }
        out.raw_copy_file(f).map_err(|e| AppError::Invalid(format!("3MF nicht schreibbar: {e}")))?;
    }
    if !ms.is_empty() {
        use std::io::Write;
        out.start_file("Metadata/model_settings.config", zip::write::SimpleFileOptions::default())
            .map_err(|e| AppError::Invalid(format!("3MF nicht schreibbar: {e}")))?;
        out.write_all(patched.as_bytes())?;
    }
    out.finish().map_err(|e| AppError::Invalid(format!("3MF nicht schreibbar: {e}")))?;
    Ok(())
}

/// Slot (`extruder`) je Objekt im Kopf des `<object>`-Blocks setzen, Teile (`<part>`) unverändert lassen
fn set_object_slots(ms: &str, slots: &[(i64, i64)]) -> String {
    let mut out = String::with_capacity(ms.len() + 64);
    let mut last = 0;
    for (i, m) in RE_OBJECT.find_iter(ms).enumerate() {
        out.push_str(&ms[last..m.start()]);
        let block = m.as_str();
        match slots.iter().find(|(idx, _)| *idx == i as i64 + 1) {
            Some((_, slot)) => {
                let split = block.find("<part").unwrap_or(block.len());
                let (head, rest) = block.split_at(split);
                let value = format!(r#"key="extruder" value="{slot}""#);
                let head = if RE_EXTR.is_match(head) {
                    RE_EXTR.replace(head, value.as_str()).into_owned()
                } else {
                    let open = head.find('>').map(|p| p + 1).unwrap_or(0);
                    format!("{}\n    <metadata {value}/>{}", &head[..open], &head[open..])
                };
                out.push_str(&head);
                out.push_str(rest);
            }
            None => out.push_str(block),
        }
        last = m.end();
    }
    out.push_str(&ms[last..]);
    out
}

/// Platten eines Projekts (aus `model_settings.config`, sonst aus den Vorschaubildern `plate_N.png`) und ihre
/// Vorschaubilder in `dest` ablegen (`plate_N.png`). Liest nur kleine Einträge der Datei, nicht die Modelle.
pub fn plates_with_images(path: &Path, dest: &Path) -> Result<Vec<(LayoutPlate, Option<std::path::PathBuf>)>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path)?).map_err(|_| AppError::Invalid("Keine gültige 3MF-Datei".into()))?;
    let names: Vec<String> = zip.file_names().map(String::from).collect();
    let mut ms = String::new();
    if let Ok(mut f) = zip.by_name("Metadata/model_settings.config") {
        let _ = f.read_to_string(&mut ms);
    }
    let mut plates: Vec<LayoutPlate> = Vec::new();
    for (i, p) in RE_PLATE.find_iter(&ms).enumerate() {
        let p = p.as_str();
        let index = RE_PLATER_ID.captures(p).and_then(|c| c[1].parse().ok()).unwrap_or(i as i64 + 1);
        let name = RE_PLATER_NAME.captures(p).map(|c| decode_entities(&c[1])).unwrap_or_default();
        plates.push(LayoutPlate { index, name });
    }
    if plates.is_empty() {
        let mut ix: Vec<i64> = names.iter().filter_map(|n| RE_PLATE_IDX.captures(n).and_then(|c| c[1].parse().ok())).collect();
        ix.sort();
        ix.dedup();
        plates = ix.into_iter().map(|index| LayoutPlate { index, name: String::new() }).collect();
    }
    std::fs::create_dir_all(dest)?;
    let mut out = Vec::new();
    for p in plates {
        let mut img = None;
        for cand in [format!("Metadata/plate_{}_small.png", p.index), format!("Metadata/plate_{}.png", p.index)] {
            if !names.contains(&cand) {
                continue;
            }
            let mut buf = Vec::new();
            if let Ok(mut f) = zip.by_name(&cand) {
                let _ = f.read_to_end(&mut buf);
            }
            if !buf.is_empty() {
                let file = dest.join(format!("plate_{}.png", p.index));
                std::fs::write(&file, &buf)?;
                img = Some(file);
                break;
            }
        }
        out.push((p, img));
    }
    Ok(out)
}

/// Slots und Objekte lesen. STL, OBJ und STEP gelten als ein Objekt ohne eigene Slots.
pub fn layout(path: &Path) -> Result<Layout> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "3mf" {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("Modell").to_string();
        return Ok(Layout { objects: vec![ModelObject { index: 1, name, slot: 1, ..Default::default() }], ..Default::default() });
    }
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| AppError::Invalid("Keine gültige 3MF-Datei".into()))?;
    let names: Vec<String> = zip.file_names().map(String::from).collect();
    let mut text = |name: &str| -> String {
        let mut buf = String::new();
        if let Ok(mut f) = zip.by_name(name) {
            let _ = f.read_to_string(&mut buf);
        }
        buf
    };
    let settings: serde_json::Value = serde_json::from_str(&text("Metadata/project_settings.config")).unwrap_or_default();
    let list = |k: &str| -> Vec<String> {
        settings[k].as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()).unwrap_or_default()
    };
    let (colors, types, presets) = (list("filament_colour"), list("filament_type"), list("filament_settings_id"));
    let n = colors.len().max(types.len()).max(presets.len());
    let mut out = Layout::default();
    for i in 0..n {
        out.slots.push(ModelSlot {
            slot: i as i64 + 1,
            material: types.get(i).cloned().unwrap_or_default(),
            color: colors.get(i).map(|c| short_color(c)).unwrap_or_default(),
            name: presets.get(i).cloned().unwrap_or_default(),
        });
    }
    let ms = text("Metadata/model_settings.config");
    for (i, c) in RE_OBJECT.captures_iter(&ms).enumerate() {
        let body = &c[1];
        let head = body.split("<part").next().unwrap_or("");
        let name = RE_NAME.captures(head).map(|m| decode_entities(&m[1])).unwrap_or_default();
        let slot = RE_EXTR.captures(head).and_then(|m| m[1].parse().ok()).unwrap_or(1);
        let mut part_slots: Vec<i64> = body.split("<part").skip(1).filter_map(|p| RE_EXTR.captures(p).and_then(|m| m[1].parse().ok())).collect();
        part_slots.sort();
        part_slots.dedup();
        let fixed = part_slots.len() > 1 || part_slots.first().is_some_and(|p| *p != slot);
        let name = if name.is_empty() { format!("Objekt {}", i + 1) } else { name };
        out.objects.push(ModelObject { index: i as i64 + 1, name, slot, fixed, parts: part_slots });
    }
    if out.objects.is_empty() {
        out.objects.push(ModelObject { index: 1, name: "Modell".into(), slot: 1, ..Default::default() });
    }
    for (i, p) in RE_PLATE.find_iter(&ms).enumerate() {
        let p = p.as_str();
        let index = RE_PLATER_ID.captures(p).and_then(|c| c[1].parse().ok()).unwrap_or(i as i64 + 1);
        let name = RE_PLATER_NAME.captures(p).map(|c| decode_entities(&c[1])).unwrap_or_default();
        out.plates.push(LayoutPlate { index, name });
    }
    for n in names.iter().filter(|n| n.starts_with("3D/") && n.ends_with(".model")) {
        if text(n).contains("paint_color=") {
            out.painted = true;
            break;
        }
    }
    Ok(out)
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

    #[test]
    fn aufbau_lesen() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.3mf");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("3D/3dmodel.model", o).unwrap();
        z.write_all(b"<model/>").unwrap();
        z.start_file("Metadata/project_settings.config", o).unwrap();
        z.write_all(br##"{"filament_colour":["#FF0000FF","#00AE42"],"filament_type":["PLA","PETG"],"filament_settings_id":["A","B"]}"##).unwrap();
        z.start_file("Metadata/model_settings.config", o).unwrap();
        z.write_all(br#"<config><object id="2"><metadata key="name" value="Deckel &amp; Fu&#223;"/><metadata key="extruder" value="2"/>
            <part id="1"><metadata key="extruder" value="2"/></part></object>
            <object id="5"><metadata key="name" value="Griff"/><metadata key="extruder" value="1"/>
            <part id="1"><metadata key="extruder" value="1"/></part><part id="2"><metadata key="extruder" value="2"/></part></object></config>"#).unwrap();
        z.finish().unwrap();
        let l = layout(&p).unwrap();
        assert_eq!(l.slots.len(), 2);
        assert_eq!((l.slots[0].color.as_str(), l.slots[1].material.as_str(), l.slots[1].name.as_str()), ("#FF0000", "PETG", "B"));
        assert_eq!(l.objects.len(), 2);
        assert_eq!((l.objects[0].name.as_str(), l.objects[0].slot, l.objects[0].fixed), ("Deckel & Fuß", 2, false));
        assert_eq!((l.objects[1].index, l.objects[1].fixed, l.objects[1].parts.clone()), (2, true, vec![1, 2]));
        assert!(!l.painted);
        // Platten ohne Einträge in model_settings: aus den Vorschaubildern
        let imgs = dir.path().join("bilder");
        assert!(plates_with_images(&p, &imgs).unwrap().is_empty());
        // Objekt-Slots in einer Kopie ändern; Teile behalten ihren Slot
        let q = dir.path().join("t2.3mf");
        with_object_slots(&p, &q, &[(1, 1), (2, 2)]).unwrap();
        let l2 = layout(&q).unwrap();
        assert_eq!((l2.objects[0].slot, l2.objects[1].slot, l2.objects[1].parts.clone()), (1, 2, vec![1, 2]));
        assert_eq!(l2.slots, l.slots, "übrige Dateien unverändert");
        assert_eq!(set_object_slots(r#"<object id="3"><metadata key="name" value="x"/></object>"#, &[(1, 4)]).matches(r#"key="extruder" value="4""#).count(), 1);
        let stl = dir.path().join("w.stl");
        std::fs::write(&stl, "solid").unwrap();
        let l = layout(&stl).unwrap();
        assert_eq!((l.objects.len(), l.slots.len(), l.objects[0].name.as_str()), (1, 0, "w.stl"));
    }

    #[test]
    fn gleiches_filament_in_zwei_slots() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("s.gcode.3mf");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("3D/3dmodel.model", o).unwrap();
        z.write_all(b"<model/>").unwrap();
        z.start_file("Metadata/slice_info.config", o).unwrap();
        z.write_all(br##"<config>
            <plate><metadata key="prediction" value="100"/><metadata key="weight" value="30"/>
              <filament id="1" type="PLA" color="#00AE42" used_g="10"/><filament id="2" type="PLA" color="#00AE42" used_g="20"/></plate>
            <plate><metadata key="prediction" value="50"/><metadata key="weight" value="5"/>
              <filament id="2" type="PLA" color="#00AE42" used_g="5"/></plate></config>"##).unwrap();
        z.finish().unwrap();
        let i = analyze(&p).unwrap();
        let got: Vec<(Option<i64>, Option<f64>)> = i.filaments.iter().map(|f| (f.slot, f.grams)).collect();
        assert_eq!(got, vec![(Some(1), Some(10.0)), (Some(2), Some(25.0))]);
        // je Platte: Zeit, Gewicht, Filamente
        assert_eq!(i.plate_list.len(), 2);
        assert_eq!((i.plate_list[0].index, i.plate_list[0].seconds, i.plate_list[0].filaments.len()), (1, Some(100), 2));
        assert_eq!((i.plate_list[1].index, i.plate_list[1].seconds, i.plate_list[1].grams), (2, Some(50), Some(5.0)));
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
