//! Kalkulation vorab: Druckprofil je Teil wählen und schätzen, bevor gesliced im Auftragsordner liegt.
//! - **Slicer:** das Teil im Hintergrund mit Bambu Studio slicen (eigene Presets aus einer Profilquelle,
//!   z.B. aidun/printerprofiles). Nur Kalkulation: Ergebnis im Arbeitsordner, kein Druck, kein Auftragsordner.
//! - **MakerWorld:** vorhergesagte Druckzeit und Gewicht eines MakerWorld-Druckprofils.

use crate::commands::{attachment_file, catalog_dir, AppState};
use crate::connectors::registry::Registry;
use crate::connectors::slicer::SliceJob;
use crate::connectors::Cap;
use crate::error::{AppError, Result};
use crate::model::{ItemEstimate, RunFilament, SetupObject, SetupSlot};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

const STUDIO: &str = "bambu-studio";

#[tauri::command]
pub async fn item_estimates(state: State<'_, AppState>, order_id: String) -> Result<Vec<ItemEstimate>> {
    state.ctx.read().await.store.order_estimates(&order_id).await
}

/// Druckprofil und Druckvorbereitung eines Teils (Slots und Objekte sind optional)
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProfileChoice {
    pub source: String,
    pub printer: String,
    pub nozzle: String,
    pub process: String,
    /// Standard-Filament (ohne Slots für alle Filamente des Modells)
    pub filament: String,
    pub bed_type: String,
    pub slots: Option<Vec<SetupSlot>>,
    pub objects: Option<Vec<SetupObject>>,
}

/// Slots und Objekte prüfen: Slots 1…16 eindeutig, Farben #RRGGBB, Objekte nur auf vorhandene Slots
fn check_setup(slots: &[SetupSlot], objects: &[SetupObject]) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for s in slots {
        if !(1..=16).contains(&s.slot) || !seen.insert(s.slot) {
            return Err(AppError::Invalid(format!("Ungültiger Filament-Slot {}.", s.slot)));
        }
        let hex = s.color.len() == 7 && s.color.starts_with('#') && s.color[1..].chars().all(|c| c.is_ascii_hexdigit());
        if !s.color.is_empty() && !hex {
            return Err(AppError::Invalid(format!("Ungültige Farbe „{}“ in Slot {}.", s.color, s.slot)));
        }
    }
    for o in objects {
        if !slots.is_empty() && !seen.contains(&o.slot) {
            return Err(AppError::Invalid(format!("„{}“ ist Slot {} zugeordnet, den es nicht gibt.", o.name, o.slot)));
        }
    }
    Ok(())
}

/// Bereit zur Übergabe: Prozess gewählt und jeder benutzte Slot hat ein Filament-Preset
fn setup_state(e: &ItemEstimate, slots: &[SetupSlot], objects: &[SetupObject]) -> &'static str {
    if slots.is_empty() {
        return "";
    }
    let used = |s: &SetupSlot| objects.is_empty() || objects.iter().any(|o| o.slot == s.slot);
    if !e.process.is_empty() && slots.iter().filter(|s| used(s)).all(|s| !s.preset.is_empty()) {
        "bereit"
    } else {
        "entwurf"
    }
}

/// Druckprofil (und ggf. Slots und Objekte) eines Teils setzen.
/// Ändert sich etwas, das das Slicen beeinflusst, wird eine bisherige Slicer-Schätzung verworfen.
#[tauri::command]
pub async fn item_set_profile(state: State<'_, AppState>, item_id: String, profile: ProfileChoice) -> Result<ItemEstimate> {
    let ProfileChoice { source, printer, nozzle, process, mut filament, bed_type, slots, objects } = profile;
    let ctx = state.ctx.read().await;
    ctx.store.get_item(&item_id).await?;
    let mut e = ctx.store.item_estimate(&item_id).await?;
    let slots_json = match &slots {
        Some(l) => {
            let mut l = l.clone();
            l.sort_by_key(|s| s.slot);
            if let Some(first) = l.iter().find(|s| !s.preset.is_empty()) {
                filament = first.preset.clone();
            }
            serde_json::to_string(&l).unwrap_or_else(|_| "[]".into())
        }
        None => e.slots.clone(),
    };
    let objects_json = match &objects {
        Some(l) => serde_json::to_string(l).unwrap_or_else(|_| "[]".into()),
        None => e.objects.clone(),
    };
    let sl: Vec<SetupSlot> = serde_json::from_str(&slots_json).unwrap_or_default();
    let ob: Vec<SetupObject> = serde_json::from_str(&objects_json).unwrap_or_default();
    check_setup(&sl, &ob)?;
    let presets = |l: &[SetupSlot]| l.iter().map(|s| s.preset.clone()).collect::<Vec<_>>();
    let ob_slots = |l: &[SetupObject]| l.iter().map(|o| o.slot).collect::<Vec<_>>();
    let old_sl: Vec<SetupSlot> = serde_json::from_str(&e.slots).unwrap_or_default();
    let old_ob: Vec<SetupObject> = serde_json::from_str(&e.objects).unwrap_or_default();
    let changed = (e.process.as_str(), e.filament.as_str(), e.bed_type.as_str()) != (process.as_str(), filament.as_str(), bed_type.as_str())
        || presets(&old_sl) != presets(&sl)
        || ob_slots(&old_ob) != ob_slots(&ob);
    e.source = source;
    e.printer = printer;
    e.nozzle = nozzle;
    e.process = process;
    e.filament = filament;
    e.bed_type = bed_type;
    e.slots = slots_json;
    e.objects = objects_json;
    if changed && e.basis == "slicer" {
        e.basis.clear();
        e.seconds = None;
        e.filaments = "[]".into();
    }
    if e.setup_state != "uebergeben" || changed {
        e.setup_state = setup_state(&e, &sl, &ob).into();
    }
    e.error.clear();
    ctx.store.save_item_estimate(&e).await?;
    ctx.store.item_estimate(&item_id).await
}

/// Vorbereitung eines Teils für den Dialog: gespeicherter Stand und Aufbau des Modells (Slots, Objekte)
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemSetup {
    pub estimate: ItemEstimate,
    pub layout: Option<crate::threemf::Layout>,
    /// warum der Aufbau fehlt (keine Modelldatei, nicht lesbar)
    pub layout_error: Option<String>,
}

#[tauri::command]
pub async fn item_setup(app: AppHandle, item_id: String) -> Result<ItemSetup> {
    let estimate = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.store.item_estimate(&item_id).await?
    };
    let (layout, layout_error) = match model_of(&app, &item_id).await {
        Ok(p) => match tokio::task::spawn_blocking(move || crate::threemf::layout(&p)).await.map_err(|e| AppError::Invalid(e.to_string()))? {
            Ok(l) => (Some(l), None),
            Err(e) => (None, Some(e.to_string())),
        },
        Err(e) => (None, Some(e.to_string())),
    };
    Ok(ItemSetup { estimate, layout, layout_error })
}

/// Modelldatei eines Teils: eigene Druckdatei, sonst die Datei des Katalogprojekts
async fn model_of(app: &AppHandle, item_id: &str) -> Result<std::path::PathBuf> {
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let item = ctx.store.get_item(item_id).await?;
    if let Some(a) = &item.file_attachment_id {
        let f = attachment_file(&ctx, a).await?;
        if f.is_file() {
            return Ok(f);
        }
    }
    if let Some(c) = &item.catalog_id {
        let e = ctx.store.get_catalog(c).await?;
        if !e.file_name.is_empty() {
            return crate::files::resolve_in(&catalog_dir(&ctx, c), &e.file_name);
        }
    }
    Err(AppError::Invalid("Das Teil hat noch keine Modelldatei (STL oder 3MF) zum Slicen.".into()))
}

/// Bambu-Druckermodell aus dem Prozess-Preset (`compatible_printers`)
fn printer_of(process: &std::path::Path) -> Result<String> {
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(process)?).map_err(|_| AppError::Invalid("Prozess-Preset nicht lesbar.".into()))?;
    v["compatible_printers"][0]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::Invalid("Im Prozess-Preset fehlt der Drucker (compatible_printers).".into()))
}

/// Teil im Hintergrund slicen und die Schätzung speichern (nur mit eingeschaltetem Schalter).
#[tauri::command]
pub async fn item_slice(app: AppHandle, registry: State<'_, Arc<Registry>>, item_id: String) -> Result<ItemEstimate> {
    if !registry.cap_on(STUDIO, Cap::SlicerSlice).await {
        return Err(AppError::Invalid("Slicen im Hintergrund ist ausgeschaltet (Einstellungen → Anbindungen → Bambu Studio).".into()));
    }
    let conn = registry.slicer().await.ok_or_else(|| AppError::Invalid("Kein Slicer eingeschaltet.".into()))?;
    let (mut est, configured, data_dir) = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        (ctx.store.item_estimate(&item_id).await?, ctx.settings.bambu_studio_path.clone(), ctx.data_dir.clone())
    };
    if est.process.is_empty() || est.filament.is_empty() {
        return Err(AppError::Invalid("Bitte zuerst Drucker, Prozess und Filament für das Teil wählen.".into()));
    }
    let model = model_of(&app, &item_id).await?;
    let process = crate::profiles::preset_file(&app, &est.process).await?;
    let filament = crate::profiles::preset_file(&app, &est.filament).await?;
    let printer = printer_of(&process)?;
    // Ein Filament-Profil für alle Slots: das des ersten Slots mit Profil, sonst das Standard-Filament.
    // Mehrere verschiedene Profile bringen die Kommandozeile bei Druckern mit Düsenwechsel (H2C)
    // zum Absturz (`group_nozzle_info`). Mit einem Profil behält Bambu Studio die Slots des Projekts
    // und liefert die Gramm je Slot; Material, Farbe und Spule kommen danach aus der Vorbereitung.
    let slots: Vec<SetupSlot> = serde_json::from_str(&est.slots).unwrap_or_default();
    let filament = match slots.iter().find(|s| !s.preset.is_empty()) {
        Some(s) => crate::profiles::preset_file(&app, &s.preset).await?,
        None => filament,
    };
    let objects: Vec<SetupObject> = serde_json::from_str(&est.objects).unwrap_or_default();
    let bed = if est.bed_type.is_empty() { "Textured PEI Plate".to_string() } else { est.bed_type.clone() };
    let work_dir = data_dir.join("slicing").join(&item_id);
    let result = tokio::task::spawn_blocking(move || {
        let slicer = conn.slicer().ok_or_else(|| AppError::Invalid("Kein Slicer.".into()))?;
        let layout = crate::threemf::layout(&model).unwrap_or_default();
        let filaments = vec![filament];
        // Objekt-Slots: bei 3MF in eine Kopie schreiben (die Kommandozeile kennt sie nur je Datei),
        // bei STL und Co. ein Wert für die eine Datei
        let is_3mf = model.extension().is_some_and(|e| e.eq_ignore_ascii_case("3mf"));
        let mut model = model;
        let mut filament_ids = Vec::new();
        if is_3mf {
            let changed: Vec<(i64, i64)> = objects
                .iter()
                .filter(|o| layout.objects.iter().any(|m| m.index == o.index && m.slot != o.slot))
                .map(|o| (o.index, o.slot))
                .collect();
            if !changed.is_empty() {
                std::fs::create_dir_all(&work_dir)?;
                let copy = work_dir.join("modell.3mf");
                crate::threemf::with_object_slots(&model, &copy, &changed)?;
                model = copy;
            }
        } else if let Some(o) = objects.first() {
            filament_ids.push(o.slot);
        }
        let job = SliceJob { model, printer, bed_type: bed, process, filaments, filament_ids, work_dir };
        let out = slicer.slice(configured.as_deref(), &job)?;
        slicer.read_project(&out)
    })
    .await
    .map_err(|e| AppError::Invalid(e.to_string()))?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    match result {
        Ok(info) if info.sliced => {
            let fils: Vec<RunFilament> = info
                .filaments
                .iter()
                .filter(|f| f.grams.unwrap_or(0.0) > 0.0)
                .map(|f| {
                    // Farbe und Spule aus dem Slot der Vorbereitung (das Preset bringt nur seine Standardfarbe mit)
                    let s = f.slot.and_then(|n| slots.iter().find(|s| s.slot == n));
                    RunFilament {
                        kind: s.filter(|s| !s.material.is_empty()).map(|s| s.material.clone()).unwrap_or_else(|| f.kind.clone()),
                        color: s.filter(|s| !s.color.is_empty()).map(|s| s.color.clone()).unwrap_or_else(|| f.color.clone()),
                        grams: (f.grams.unwrap_or(0.0) * 100.0).round() / 100.0,
                        spool_id: s.and_then(|s| s.spool_id.clone()),
                        slot: f.slot,
                    }
                })
                .collect();
            est.basis = "slicer".into();
            est.seconds = info.print_seconds;
            est.filaments = serde_json::to_string(&fils).unwrap_or_else(|_| "[]".into());
            est.error.clear();
        }
        Ok(_) => est.error = "Bambu Studio hat kein gesliced Ergebnis geliefert.".into(),
        Err(e) => est.error = e.to_string(),
    }
    ctx.store.save_item_estimate(&est).await?;
    if !est.error.is_empty() {
        return Err(AppError::Invalid(est.error));
    }
    Ok(est)
}

/// Druckprofile von MakerWorld zum Katalogprojekt eines Teils
#[tauri::command]
pub async fn item_makerworld_profiles(app: AppHandle, item_id: String) -> Result<Vec<crate::bambu::makerworld::MwProfile>> {
    let remote = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        let item = ctx.store.get_item(&item_id).await?;
        let c = item.catalog_id.ok_or_else(|| AppError::Invalid("Das Teil stammt nicht aus dem Katalog.".into()))?;
        ctx.store.get_catalog(&c).await?.remote_id.ok_or_else(|| AppError::Invalid("Das Katalogprojekt stammt nicht von MakerWorld.".into()))?
    };
    crate::bambu::makerworld::design_profiles(&app, &remote).await
}

/// Schätzung aus einem MakerWorld-Druckprofil übernehmen (Material aus dem Katalogprojekt, falls bekannt).
#[tauri::command]
pub async fn item_estimate_makerworld(app: AppHandle, item_id: String, profile_id: String) -> Result<ItemEstimate> {
    let profiles = item_makerworld_profiles(app.clone(), item_id.clone()).await?;
    let p = profiles.into_iter().find(|p| p.id == profile_id).ok_or_else(|| AppError::NotFound("Druckprofil".into()))?;
    if p.seconds.is_none() && p.grams.is_none() {
        return Err(AppError::Invalid("Dieses Druckprofil nennt weder Druckzeit noch Gewicht.".into()));
    }
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let item = ctx.store.get_item(&item_id).await?;
    let material = match &item.catalog_id {
        Some(c) => serde_json::from_str::<Vec<serde_json::Value>>(&ctx.store.get_catalog(c).await?.filaments)
            .ok()
            .and_then(|l| l.first().and_then(|f| f["type"].as_str().map(str::to_string)))
            .unwrap_or_default(),
        None => String::new(),
    };
    let mut e = ctx.store.item_estimate(&item_id).await?;
    e.basis = "makerworld".into();
    e.seconds = p.seconds;
    e.mw_instance = p.id;
    e.filaments = serde_json::to_string(&vec![RunFilament { kind: material, color: String::new(), grams: p.grams.unwrap_or(0.0), spool_id: None, slot: None }])
        .unwrap_or_else(|_| "[]".into());
    e.error.clear();
    ctx.store.save_item_estimate(&e).await?;
    ctx.store.item_estimate(&item_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vorbereitung_pruefen_und_status() {
        let slot = |n: i64, preset: &str| SetupSlot { slot: n, preset: preset.into(), color: "#FF0000".into(), ..Default::default() };
        let obj = |slot: i64| SetupObject { index: 1, name: "A".into(), slot };
        assert!(check_setup(&[slot(1, ""), slot(2, "")], &[obj(2)]).is_ok());
        assert!(check_setup(&[slot(1, ""), slot(1, "")], &[]).is_err(), "doppelter Slot");
        assert!(check_setup(&[slot(1, "")], &[obj(3)]).is_err(), "Objekt auf fehlendem Slot");
        assert!(check_setup(&[SetupSlot { slot: 1, color: "rot".into(), ..Default::default() }], &[]).is_err());
        let e = ItemEstimate { process: "p".into(), ..Default::default() };
        assert_eq!(setup_state(&e, &[], &[]), "");
        assert_eq!(setup_state(&e, &[slot(1, "f"), slot(2, "")], &[obj(1)]), "bereit", "unbenutzter Slot ohne Preset stört nicht");
        assert_eq!(setup_state(&e, &[slot(1, "f"), slot(2, "")], &[]), "entwurf");
        assert_eq!(setup_state(&ItemEstimate::default(), &[slot(1, "f")], &[]), "entwurf", "ohne Prozess");
    }

    #[test]
    fn drucker_aus_prozess_preset() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("p.json");
        std::fs::write(&f, r#"{"compatible_printers":["Bambu Lab H2C 0.4 nozzle"],"inherits":""}"#).unwrap();
        assert_eq!(printer_of(&f).unwrap(), "Bambu Lab H2C 0.4 nozzle");
        std::fs::write(&f, r#"{"name":"x"}"#).unwrap();
        assert!(printer_of(&f).is_err());
    }
}
