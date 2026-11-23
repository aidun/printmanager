//! Bambu Studio als Slicer-Anbindung `bambu-studio` (docs/PLAN-Anbindungen.md, Stufe A5):
//! Programm finden und Dateien öffnen, Filament aus `filament_inventory/spools.json` lesen (nie schreiben),
//! gesliced 3MF lesen (`threemf.rs`, Format von Bambu Studio und OrcaSlicer).

use crate::connectors::slicer::{SliceJob, SlicerAdapter};
use crate::connectors::{AuthKind, Cap, ConnCtx, Connector, Descriptor, Kind, Platform, Status, SyncOutcome};
use crate::error::{AppError, Result};
use crate::model::StudioSpool;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

pub static DESCRIPTOR: Descriptor = Descriptor {
    id: "bambu-studio",
    name: "Bambu Studio",
    kind: Kind::Slicer,
    platforms: &[Platform::MacOs, Platform::Windows, Platform::Linux],
    auth: AuthKind::None,
    caps: &[Cap::SlicerOpen, Cap::SpoolsRead, Cap::SlicerSlice],
    base_url: "",
    write_paths: &[],
    read_post_paths: &[],
    timeout_secs: 0,
    default_enabled: true,
    min_interval_ms: 0,
    intervals: &[],
    secrets: &[],
    heartbeat_secs: 0,
};

pub struct BambuStudio;

impl SlicerAdapter for BambuStudio {
    fn program_name(&self) -> &'static str {
        "Bambu Studio"
    }

    fn program_candidates(&self) -> &'static [&'static str] {
        if cfg!(target_os = "macos") {
            &["/Applications/BambuStudio.app", "/Applications/Bambu Studio.app"]
        } else if cfg!(target_os = "windows") {
            &[r"C:\Program Files\Bambu Studio\bambu-studio.exe"]
        } else {
            &["/usr/bin/bambu-studio", "/usr/local/bin/bambu-studio"]
        }
    }

    /// macOS: `~/Library/Application Support`, Windows: `%APPDATA%`
    fn default_spools_file(&self, system_data_dir: &Path) -> Option<PathBuf> {
        Some(system_data_dir.join("BambuStudio").join("filament_inventory").join("spools.json"))
    }

    fn read_spools(&self, file: &Path) -> Result<Option<Vec<StudioSpool>>> {
        read(file)
    }

    fn read_project(&self, file: &Path) -> Result<crate::threemf::Info> {
        crate::threemf::analyze(file)
    }

    /// Kommandozeile von Bambu Studio: `--slice 0` mit Druckerprofil, Prozess- und Filament-Presets.
    /// Die Presets müssen eigenständig sein (die Kommandozeile löst `inherits` nicht auf);
    /// das kleine Druckerprofil erbt nur das Bambu-Druckermodell und setzt die Druckplatte.
    fn slice(&self, configured: Option<&str>, job: &SliceJob) -> Result<PathBuf> {
        let app = self.find_program(configured).ok_or_else(|| AppError::Invalid("Bambu Studio wurde nicht gefunden.".into()))?;
        let exe = cli_binary(&app);
        std::fs::create_dir_all(&job.work_dir)?;
        let machine = job.work_dir.join("drucker.json");
        std::fs::write(&machine, serde_json::to_vec_pretty(&machine_profile(&job.printer, &job.bed_type)).unwrap_or_default())?;
        let out = job.work_dir.join(RESULT);
        let _ = std::fs::remove_file(&out);
        let join = |l: &[PathBuf]| l.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(";");
        let log = std::fs::File::create(job.work_dir.join("bambu-studio.log"))?;
        let mut cmd = std::process::Command::new(&exe);
        if !job.filament_ids.is_empty() {
            cmd.arg("--load-filament-ids").arg(job.filament_ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        }
        let mut child = cmd
            .arg("--slice")
            .arg("0")
            .arg("--arrange")
            .arg("1")
            // Modelle um den Nullpunkt (häufig bei STL) liegen sonst zum Teil unter der Platte:
            // „Nothing to be sliced … no object fully inside the print volume“
            .arg("--ensure-on-bed")
            .arg("--load-settings")
            .arg(format!("{};{}", machine.display(), job.process.display()))
            .arg("--load-filaments")
            .arg(join(&job.filaments))
            // Dateiname relativ zu --outputdir; ein absoluter Pfad wird angehängt und schlägt fehl
            .arg("--export-3mf")
            .arg(RESULT)
            .arg("--outputdir")
            .arg(&job.work_dir)
            .arg(&job.model)
            .current_dir(&job.work_dir)
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .map_err(|e| AppError::Invalid(format!("Bambu Studio startet nicht: {e}")))?;
        // höchstens 10 Minuten
        let start = std::time::Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait()? {
                break s;
            }
            if start.elapsed() > std::time::Duration::from_secs(600) {
                let _ = child.kill();
                return Err(AppError::Invalid("Slicen dauerte zu lange (über 10 Minuten).".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        };
        if !status.success() || !out.is_file() {
            let tail = std::fs::read_to_string(job.work_dir.join("bambu-studio.log")).unwrap_or_default();
            let last: Vec<&str> = tail.lines().filter(|l| !l.trim().is_empty()).rev().take(3).collect();
            return Err(AppError::Invalid(format!(
                "Bambu Studio konnte nicht slicen ({}). {}",
                status.code().map(|c| format!("Code {c}")).unwrap_or_else(|| "abgebrochen".into()),
                last.into_iter().rev().collect::<Vec<_>>().join(" · ")
            )));
        }
        Ok(out)
    }
}

/// Ergebnis des Slicens im Arbeitsordner
const RESULT: &str = "ergebnis.gcode.3mf";

/// Druckerprofil für die Kommandozeile: erbt vom Bambu-Drucker. Ohne `type` lehnt Bambu Studio die Datei ab,
/// ohne `curr_bed_type` bricht es bei Materialien ohne Cool-Plate-Temperatur ab.
fn machine_profile(printer: &str, bed_type: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "machine",
        "name": format!("Printmanager {printer}"),
        "from": "User",
        "inherits": printer,
        "curr_bed_type": bed_type,
    })
}

/// Ausführbare Datei für die Kommandozeile: unter macOS im App-Bundle
fn cli_binary(app: &Path) -> PathBuf {
    if app.extension().is_some_and(|e| e == "app") {
        app.join("Contents").join("MacOS").join("BambuStudio")
    } else {
        app.to_path_buf()
    }
}

#[async_trait]
impl Connector for BambuStudio {
    fn descriptor(&self) -> &'static Descriptor {
        &DESCRIPTOR
    }
    /// Lokales Programm: immer „verbunden“; ob es installiert ist, zeigt `find_program`.
    async fn status(&self, _ctx: &ConnCtx) -> Status {
        Status { connected: true, ..Default::default() }
    }
    async fn connect(&self, _ctx: &ConnCtx) -> Result<()> {
        Ok(())
    }
    async fn sync(&self, _ctx: &ConnCtx, _cap: Cap) -> Result<SyncOutcome> {
        Ok(SyncOutcome::default())
    }
    fn slicer(&self) -> Option<&dyn SlicerAdapter> {
        Some(self)
    }
}

// ---------- spools.json ----------

pub fn parse(json: &str) -> Result<Vec<StudioSpool>> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| AppError::Invalid(format!("spools.json nicht lesbar: {e}")))?;
    let list = v["spools"].as_array().ok_or_else(|| AppError::Invalid("spools.json: Liste „spools“ fehlt".into()))?;
    let s = |x: &serde_json::Value| x.as_str().unwrap_or("").trim().to_string();
    let f = |x: &serde_json::Value| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0);
    Ok(list
        .iter()
        .filter_map(|sp| {
            let id = match &sp["spool_id"] {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => return None,
            };
            let mut c = crate::filament::color(&s(&sp["color_code"]));
            if c.is_empty() {
                c = sp["colors"].as_array().and_then(|a| a.first()).map(|x| crate::filament::color(&s(x))).unwrap_or_default();
            }
            Some(StudioSpool {
                id,
                brand: s(&sp["brand"]),
                series: s(&sp["series"]),
                material: s(&sp["material_type"]),
                color: c,
                color_name: s(&sp["color_name"]),
                initial_grams: f(&sp["initial_weight"]),
                net_grams: f(&sp["net_weight"]),
                status: s(&sp["status"]),
                source: "datei".into(),
                rfid: s(&sp["tag_uid"]),
                ..Default::default()
            })
        })
        .collect())
}


pub fn read(file: &Path) -> Result<Option<Vec<StudioSpool>>> {
    match std::fs::read_to_string(file) {
        Ok(s) => {
            let changed = std::fs::metadata(file)
                .and_then(|m| m.modified())
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            let mut list = parse(&s)?;
            for sp in &mut list {
                sp.source_updated_at = changed.clone();
            }
            Ok(Some(list))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kommandozeile_im_bundle() {
        assert_eq!(cli_binary(Path::new("/Applications/BambuStudio.app")), Path::new("/Applications/BambuStudio.app/Contents/MacOS/BambuStudio"));
        assert_eq!(cli_binary(Path::new("C:/x/bambu-studio.exe")), Path::new("C:/x/bambu-studio.exe"));
        let m = machine_profile("Bambu Lab H2C 0.4 nozzle", "Textured PEI Plate");
        assert_eq!((m["type"].as_str(), m["inherits"].as_str(), m["curr_bed_type"].as_str()), (Some("machine"), Some("Bambu Lab H2C 0.4 nozzle"), Some("Textured PEI Plate")));
    }

    #[test]
    fn programm_und_dateiort() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("BambuStudio.app");
        std::fs::create_dir(&exe).unwrap();
        let s = BambuStudio;
        // eingestellter Pfad gilt, wenn er existiert; ein falscher Pfad findet nichts (kein Rückfall)
        assert_eq!(s.find_program(exe.to_str()), Some(exe.clone()));
        assert_eq!(s.find_program(Some("/gibt/es/nicht")), None);
        assert_eq!(
            s.default_spools_file(Path::new("/daten")).unwrap(),
            Path::new("/daten").join("BambuStudio").join("filament_inventory").join("spools.json")
        );
        // fehlende Datei: kein Fehler, nur „nichts da“
        assert!(s.read_spools(&dir.path().join("spools.json")).unwrap().is_none());
        std::fs::write(dir.path().join("spools.json"), r#"{"spools":[{"spool_id":"1","material_type":"PLA","net_weight":100}]}"#).unwrap();
        let l = s.read_spools(&dir.path().join("spools.json")).unwrap().unwrap();
        assert_eq!((l.len(), l[0].source.as_str()), (1, "datei"));
        assert!(l[0].source_updated_at.is_some(), "Änderungszeit der Datei als Stand");
        assert!(s.open(Some("/gibt/es/nicht"), Path::new("/x.3mf")).unwrap_err().to_string().contains("Bambu Studio wurde nicht gefunden"));
    }

    #[tokio::test]
    async fn registry_findet_den_slicer() {
        use crate::connectors::registry::Registry;
        use crate::connectors::secrets::MemoryBackend;
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let r = Registry::new(dir.path().to_path_buf(), Arc::new(MemoryBackend::default()), vec![Arc::new(BambuStudio)]).unwrap();
        let c = r.slicer().await.expect("ab Werk an");
        assert_eq!(c.slicer().unwrap().program_name(), "Bambu Studio");
        assert!(r.get("bambu-studio").unwrap().ctx.http.is_none(), "lokales Programm ohne HTTP-Zugang");
        r.set_cap("bambu-studio", Cap::SlicerOpen, false).await.unwrap();
        assert!(r.slicer().await.is_none());
    }

    #[test]
    fn spulen_lesen() {
        let json = r##"{"spools":[
            {"spool_id":"10897774","brand":"Bambu Lab","series":"PLA Matte","material_type":"PLA",
             "color_code":"#0078BFFF","colors":["#0078BFFF"],"initial_weight":1000.0,"net_weight":340.0,"status":"active"},
            {"spool_id":6226783,"brand":"SUNLU","series":"PLA+ 2.0","material_type":"PLA",
             "color_code":"","colors":["#fff144ff"],"initial_weight":1000,"net_weight":"500","status":"active"},
            {"brand":"ohne id"}
        ]}"##;
        let l = parse(json).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].color.as_str(), l[0].net_grams), ("#0078BF", 340.0));
        assert_eq!((l[1].id.as_str(), l[1].color.as_str(), l[1].net_grams), ("6226783", "#FFF144", 500.0));
        assert!(parse("{}").is_err());
        assert_eq!(l[0].source, "datei");
    }
}
