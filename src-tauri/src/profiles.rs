//! Druckprofile (Presets) für den Slicer als Anbindung. Erste Quelle: das öffentliche Repository
//! `aidun/printerprofiles` (eigenständige Presets für Filament und Prozess, je Drucker und Düse).
//! Index und Presets werden im Datenordner unter `profiles/<anbindung>/` zwischengespeichert; der Index wird
//! täglich aufgefrischt, Presets beim ersten Gebrauch geladen. Weitere Quellen sind weitere Descriptoren.

use crate::commands::AppState;
use crate::connectors::http::HttpGateway;
use crate::connectors::registry::Registry;
use crate::connectors::{AuthKind, Cap, ConnCtx, Connector, Descriptor, Kind, Platform, Status, SyncOutcome};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub static PRINTERPROFILES: Descriptor = Descriptor {
    id: "printerprofiles",
    name: "Druckprofile (aidun/printerprofiles)",
    kind: Kind::Slicer,
    platforms: &[Platform::MacOs, Platform::Windows, Platform::Linux],
    auth: AuthKind::None,
    caps: &[Cap::ProfilesRead],
    base_url: "https://raw.githubusercontent.com",
    write_paths: &[],
    read_post_paths: &[],
    timeout_secs: 30,
    default_enabled: true,
    min_interval_ms: 200,
    intervals: &[(Cap::ProfilesRead, 24 * 3600)],
    secrets: &[],
    heartbeat_secs: 0,
};

/// Ordner der fertigen Presets im Repository
const DIST: &str = "/aidun/printerprofiles/main/dist";

/// Eintrag im Index (`dist/_index.json`); Felder tolerant
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Preset {
    /// filament | process
    pub kind: String,
    pub name: String,
    pub file: String,
    /// Drucker-Kennung, z.B. h2c
    pub printer: String,
    pub nozzle: String,
    /// Bambu-Basisprofil
    pub base: String,
    /// Filament: Material-Kennung, z.B. bambu-pla-basic
    pub filament: Option<String>,
    pub abrasive: Option<bool>,
    /// Prozess: quality | normal | fast
    pub level: Option<String>,
    pub support: Option<bool>,
    #[serde(alias = "layer_height")]
    pub layer_height: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileIndex {
    pub presets: Vec<Preset>,
}

/// Bambu-Druckermodell zur Drucker-Kennung (für Anzeige und das Druckerprofil der Kommandozeile)
pub fn printer_label(id: &str) -> String {
    match id {
        "h2c" => "H2C".into(),
        "h3c" => "H3C".into(),
        "x2d" => "X2D".into(),
        "x1c" => "X1 Carbon".into(),
        "p1s" => "P1S".into(),
        "p1p" => "P1P".into(),
        "a1" => "A1".into(),
        "a1mini" => "A1 mini".into(),
        other => other.to_uppercase(),
    }
}

/// Pfad im Repository: jedes Segment URL-kodiert (Namen enthalten Leerzeichen und Umlaute)
fn url_path(file: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!("{DIST}/{}", file.split('/').map(enc).collect::<Vec<_>>().join("/"))
}

/// Datei liegt sicher im Cache-Ordner (keine `..`, kein absoluter Pfad)
fn safe_rel(file: &str) -> Result<&str> {
    if file.is_empty() || file.starts_with('/') || file.split('/').any(|p| p == ".." || p.is_empty()) {
        return Err(AppError::Invalid("Ungültiger Preset-Pfad".into()));
    }
    Ok(file)
}

pub struct ProfileSource {
    pub app: AppHandle,
}

impl ProfileSource {
    async fn cache_dir(&self) -> PathBuf {
        let state = self.app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.data_dir.join("profiles").join(PRINTERPROFILES.id)
    }
}

async fn http_of(reg: &Registry) -> Result<Arc<HttpGateway>> {
    reg.get(PRINTERPROFILES.id)?.ctx.http.clone().ok_or_else(|| AppError::Invalid("Druckprofile: kein HTTP-Zugang".into()))
}

/// Index laden: aus dem Cache, sonst herunterladen. `refresh` lädt neu.
pub async fn index(app: &AppHandle, refresh: bool) -> Result<ProfileIndex> {
    let reg = app.state::<Arc<Registry>>();
    if !reg.cap_on(PRINTERPROFILES.id, Cap::ProfilesRead).await {
        return Err(AppError::Invalid("Die Druckprofile sind ausgeschaltet (Einstellungen → Anbindungen).".into()));
    }
    let dir = ProfileSource { app: app.clone() }.cache_dir().await;
    let file = dir.join("_index.json");
    if !refresh {
        if let Ok(s) = std::fs::read_to_string(&file) {
            if let Ok(i) = serde_json::from_str::<ProfileIndex>(&s) {
                return Ok(i);
            }
        }
    }
    let bytes = http_of(&reg).await?.get_bytes(&url_path("_index.json"), None).await?;
    let i: ProfileIndex = serde_json::from_slice(&bytes).map_err(|_| AppError::Invalid("Index der Druckprofile nicht lesbar.".into()))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&file, &bytes)?;
    Ok(i)
}

/// Preset-Datei lokal bereitstellen (Cache, sonst herunterladen).
pub async fn preset_file(app: &AppHandle, file: &str) -> Result<PathBuf> {
    let rel = safe_rel(file)?;
    let dir = ProfileSource { app: app.clone() }.cache_dir().await;
    let path = dir.join(rel);
    if path.is_file() {
        return Ok(path);
    }
    let reg = app.state::<Arc<Registry>>();
    let bytes = http_of(&reg).await?.get_bytes(&url_path(rel), None).await?;
    serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|_| AppError::Invalid(format!("Preset „{file}“ nicht lesbar.")))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &bytes)?;
    Ok(path)
}

#[async_trait]
impl Connector for ProfileSource {
    fn descriptor(&self) -> &'static Descriptor {
        &PRINTERPROFILES
    }
    async fn status(&self, _ctx: &ConnCtx) -> Status {
        Status { connected: true, ..Default::default() }
    }
    async fn connect(&self, _ctx: &ConnCtx) -> Result<()> {
        Ok(())
    }
    /// Index täglich auffrischen; geladene Presets neu holen lassen, wenn sich der Index geändert hat.
    async fn sync(&self, _ctx: &ConnCtx, cap: Cap) -> Result<SyncOutcome> {
        if cap != Cap::ProfilesRead {
            return Ok(SyncOutcome::default());
        }
        let dir = self.cache_dir().await;
        let old = std::fs::read(dir.join("_index.json")).ok();
        index(&self.app, true).await?;
        let new = std::fs::read(dir.join("_index.json")).ok();
        if old.is_some() && old != new {
            for sub in ["filament", "process"] {
                let _ = std::fs::remove_dir_all(dir.join(sub));
            }
        }
        Ok(SyncOutcome::default())
    }
}

// ---------- Befehle ----------

/// Auswahl für die Oberfläche: Drucker mit Düsen, Prozess- und Filament-Presets
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCatalog {
    pub source: &'static str,
    pub printers: Vec<PrinterChoice>,
    pub presets: Vec<Preset>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterChoice {
    pub id: String,
    pub label: String,
    pub nozzles: Vec<String>,
}

pub fn catalog_of(i: &ProfileIndex) -> ProfileCatalog {
    let mut printers: Vec<PrinterChoice> = Vec::new();
    for p in &i.presets {
        match printers.iter_mut().find(|x| x.id == p.printer) {
            Some(x) => {
                if !x.nozzles.contains(&p.nozzle) {
                    x.nozzles.push(p.nozzle.clone());
                }
            }
            None => printers.push(PrinterChoice { id: p.printer.clone(), label: printer_label(&p.printer), nozzles: vec![p.nozzle.clone()] }),
        }
    }
    for p in &mut printers {
        p.nozzles.sort();
    }
    printers.sort_by(|a, b| a.label.cmp(&b.label));
    ProfileCatalog { source: PRINTERPROFILES.id, printers, presets: i.presets.clone() }
}

#[tauri::command]
pub async fn slice_profiles(app: AppHandle, refresh: Option<bool>) -> Result<ProfileCatalog> {
    Ok(catalog_of(&index(&app, refresh.unwrap_or(false)).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfade_und_index() {
        assert_eq!(url_path("process/Normal + Stützen H2C 0.4.json"), "/aidun/printerprofiles/main/dist/process/Normal%20%2B%20St%C3%BCtzen%20H2C%200.4.json");
        assert!(safe_rel("../x").is_err() && safe_rel("/etc/x").is_err() && safe_rel("a//b").is_err());
        let i: ProfileIndex = serde_json::from_str(
            r#"{"presets":[
                {"kind":"process","name":"Normal H2C 0.4","file":"process/Normal H2C 0.4.json","printer":"h2c","nozzle":"0.4","base":"0.20mm Standard @BBL H2C","level":"normal","support":false,"layer_height":"0.2"},
                {"kind":"filament","name":"Bambu PLA Basic H2C 0.6","file":"filament/x.json","printer":"h2c","nozzle":"0.6","base":"b","filament":"bambu-pla-basic","abrasive":false,"neu":1},
                {"kind":"process","name":"Normal A1M 0.4","file":"p","printer":"a1mini","nozzle":"0.4","base":"b"}
            ],"library":"/x"}"#,
        )
        .unwrap();
        assert_eq!(i.presets[0].layer_height.as_deref(), Some("0.2"));
        let c = catalog_of(&i);
        assert_eq!(c.printers.len(), 2);
        let h2c = c.printers.iter().find(|p| p.id == "h2c").unwrap();
        assert_eq!((h2c.label.as_str(), h2c.nozzles.clone()), ("H2C", vec!["0.4".to_string(), "0.6".to_string()]));
        assert_eq!(printer_label("a1mini"), "A1 mini");
    }
}
