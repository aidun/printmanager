//! Spulen ins Bambu-Konto schreiben (abschaltbar, ab Werk aus). Bestätigt im Test vom 17.11.2026:
//! `PUT /my/filament/v2` (Teilaktualisierung, Gramm als ganze Zahl, `filamentName` Pflicht),
//! `POST /my/filament/v2` (Antwort `{}`), `DELETE /my/filament/v2/batch` mit `{"ids": [...]}`.
//!
//! Regeln: nur bei eingeschaltetem Schalter; vor jedem Schreiben frisch lesen und bei fremder Änderung
//! (Konflikt) nicht überschreiben; RFID-Spulen: kein Rest (pflegt das AMS); Spule im druckenden Drucker:
//! Änderung wartet bis nach dem Druck; alles wird protokolliert.

use super::Bambu;
use crate::commands::AppState;
use crate::error::{AppError, Result};
use crate::model::{FilamentWrite, Spool};
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

const LIST: &str = "/v1/design-user-service/my/filament/v2";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    /// ok | konflikt | rfid | wartet | fehler
    pub status: String,
    pub message: String,
    /// Rest laut Konto (bei Konflikt der fremde Wert)
    pub cloud_grams: Option<f64>,
}

fn res(status: &str, message: impl Into<String>, cloud: Option<f64>) -> WriteResult {
    WriteResult { status: status.into(), message: message.into(), cloud_grams: cloud }
}

fn label(s: &Spool) -> String {
    [s.brand.as_str(), s.series.as_str(), s.color.as_str()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

/// Konto-Zeitstempel (Sekunden) wie beim Abgleich als RFC 3339
fn stamp(v: &Value) -> Option<String> {
    v["updatedAt"].as_i64().and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

/// Wurde die Spule im Konto seit dem letzten Abgleich verändert (Bambu Studio, Handy, AMS)?
fn conflict(cloud: &Value, spool: &Spool) -> bool {
    let changed = stamp(cloud) != spool.source_updated_at;
    let differs = cloud["netWeight"].as_f64().map(|g| (g - spool.studio_grams).abs() > 0.5).unwrap_or(false);
    changed && differs
}

async fn ensure_enabled(b: &Bambu) -> Result<()> {
    let st = b.status().await;
    if !st.connected {
        return Err(AppError::Invalid("Nicht am Bambu-Konto angemeldet.".into()));
    }
    if !st.config.filament_write {
        return Err(AppError::Invalid("Schreiben ins Bambu-Konto ist ausgeschaltet (Einstellungen → Bambu-Konto).".into()));
    }
    Ok(())
}

/// Eine Spule frisch aus dem Konto lesen (erst gezielt, sonst über alle Seiten).
pub async fn cloud_spool(b: &Bambu, id: &str) -> Result<Option<Value>> {
    let hit = |v: &Value| v["hits"].as_array().and_then(|a| a.iter().find(|s| s["id"].as_i64().map(|n| n.to_string()).as_deref() == Some(id) || s["id"].as_str() == Some(id)).cloned());
    if let Ok(v) = b.get(&format!("{LIST}?ids={id}")).await {
        if let Some(s) = hit(&v) {
            return Ok(Some(s));
        }
    }
    let mut offset = 0;
    loop {
        let v = b.get(&format!("{LIST}?offset={offset}&limit=50")).await?;
        if let Some(s) = hit(&v) {
            return Ok(Some(s));
        }
        let n = v["hits"].as_array().map(|a| a.len()).unwrap_or(0);
        offset += 50;
        if n < 50 || offset >= v["total"].as_i64().unwrap_or(0) || offset > 2000 {
            return Ok(None);
        }
    }
}

/// Druckt gerade ein Drucker, in dem die Spule steckt?
async fn printing_with(app: &AppHandle, s: &Spool) -> bool {
    if s.in_printer == 0 {
        return false;
    }
    let b = app.state::<Bambu>();
    let printers = b.printers.read().await;
    printers.iter().any(|p| super::printers::active(&p.state) && (s.printer_name.is_empty() || p.name.eq_ignore_ascii_case(&s.printer_name)))
}

/// Rest einer Spule ins Konto schreiben. `force`: auch bei Konflikt schreiben.
pub async fn push_rest(app: &AppHandle, id: &str, grams: f64, force: bool) -> Result<WriteResult> {
    let b = app.state::<Bambu>();
    ensure_enabled(&b).await?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let spool = ctx.store.get_spool(id).await?;
    let grams = grams.max(0.0).round();
    let lbl = label(&spool);
    let old = Some(format!("{:.0}", spool.studio_grams));
    let new = Some(format!("{grams:.0}"));
    if spool.source != "konto" {
        return Ok(res("fehler", "Die Spule stammt nicht aus dem Bambu-Konto.", None));
    }
    if !spool.rfid.is_empty() {
        ctx.store.log_filament_write(id, &lbl, "rest", old, new, "uebersprungen", "RFID-Spule: den Rest pflegt das AMS", None).await?;
        return Ok(res("rfid", "Bambu-Spule mit RFID – den Rest pflegt das AMS, er wird nicht ins Konto geschrieben.", None));
    }
    if printing_with(app, &spool).await {
        ctx.store.log_filament_write(id, &lbl, "rest", old, new, "wartet", "Spule steckt im druckenden Drucker", None).await?;
        return Ok(res("wartet", "Die Spule steckt im Drucker, der gerade druckt – wird nach dem Druck ins Konto geschrieben.", None));
    }
    let Some(cloud) = cloud_spool(&b, id).await? else {
        ctx.store.log_filament_write(id, &lbl, "rest", old, new, "fehler", "Spule nicht mehr im Konto", None).await?;
        return Ok(res("fehler", "Die Spule gibt es im Bambu-Konto nicht mehr.", None));
    };
    let cloud_grams = cloud["netWeight"].as_f64();
    // Konflikt: im Konto seit dem letzten Abgleich verändert (Bambu Studio/Handy/AMS)
    if !force && conflict(&cloud, &spool) {
        ctx.store
            .log_filament_write(id, &lbl, "rest", old, new, "konflikt", &format!("im Konto inzwischen {:.0} g", cloud_grams.unwrap_or(0.0)), Some(cloud.to_string()))
            .await?;
        return Ok(res("konflikt", "Die Spule wurde inzwischen in Bambu Studio oder Handy geändert.", cloud_grams));
    }
    let body = json!({ "id": cloud["id"], "filamentName": cloud["filamentName"], "netWeight": grams as i64 });
    let (status, text) = b.send_raw(Method::PUT, LIST, Some(&body)).await?;
    if !(200..300).contains(&status) {
        let msg = format!("Konto antwortet {status}: {}", text.chars().take(160).collect::<String>());
        ctx.store.log_filament_write(id, &lbl, "rest", old, new, "fehler", &msg, Some(cloud.to_string())).await?;
        return Ok(res("fehler", msg, cloud_grams));
    }
    // zur Kontrolle neu lesen
    let after = cloud_spool(&b, id).await?.unwrap_or(Value::Null);
    let ok = after["netWeight"].as_f64().map(|g| (g - grams).abs() < 0.6).unwrap_or(false);
    ctx.store.spool_written(id, grams, stamp(&after).as_deref()).await?;
    ctx.store
        .log_filament_write(id, &lbl, "rest", old, new, if ok { "ok" } else { "fehler" }, if ok { "" } else { "Wert im Konto weicht ab" }, Some(cloud.to_string()))
        .await?;
    Ok(if ok { res("ok", format!("{lbl}: {grams:.0} g im Bambu-Konto gespeichert"), Some(grams)) } else { res("fehler", "Gespeichert, aber das Konto zeigt einen anderen Wert.", after["netWeight"].as_f64()) })
}

/// Wartende Änderungen nach dem Druck schreiben (vom Takt aufgerufen).
pub async fn process_pending(app: &AppHandle) -> Vec<String> {
    let b = app.state::<Bambu>();
    if ensure_enabled(&b).await.is_err() {
        return vec![];
    }
    let pending: Vec<FilamentWrite> = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.store.list_filament_writes(200).await.unwrap_or_default()
    };
    // je Spule nur der neueste Eintrag zählt
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for w in pending {
        if !seen.insert(w.spool_id.clone()) || w.status != "wartet" || w.action != "rest" {
            continue;
        }
        let Some(g) = w.new_value.as_deref().and_then(|x| x.parse::<f64>().ok()) else { continue };
        if let Ok(r) = push_rest(app, &w.spool_id, g, false).await {
            if r.status == "ok" || r.status == "konflikt" {
                out.push(if r.status == "ok" { r.message } else { format!("{}: Konflikt beim Zurückschreiben – bitte auf der Filament-Seite prüfen", w.label) });
            }
        }
    }
    out
}

/// Vorrat erhöhen: eine gleichartige, volle Spule im Konto anlegen.
pub async fn stock_add(app: &AppHandle, id: &str) -> Result<WriteResult> {
    let b = app.state::<Bambu>();
    ensure_enabled(&b).await?;
    let Some(cloud) = cloud_spool(&b, id).await? else { return Ok(res("fehler", "Vorlage nicht im Konto gefunden.", None)) };
    let mut body = cloud.clone();
    if let Some(o) = body.as_object_mut() {
        for k in ["id", "createdAt", "updatedAt", "RFID", "inPrinter", "devId", "amsSn", "amsId", "amsType", "slotId", "trayIdName", "deviceName", "depleted"] {
            o.remove(k);
        }
    }
    let total = cloud["totalNetWeight"].as_f64().unwrap_or(1000.0).round() as i64;
    body["netWeight"] = json!(total);
    body["createType"] = json!("manual");
    let (status, text) = b.send_raw(Method::POST, LIST, Some(&body)).await?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let spool = ctx.store.get_spool(id).await?;
    let ok = (200..300).contains(&status);
    ctx.store
        .log_filament_write(id, &label(&spool), "anlegen", None, Some(format!("{total}")), if ok { "ok" } else { "fehler" },
            if ok { "neue volle Spule nach Vorlage" } else { &text }, Some(body.to_string()))
        .await?;
    Ok(if ok { res("ok", format!("Neue Spule {} ({total} g) im Bambu-Konto angelegt", label(&spool)), None) } else { res("fehler", format!("Konto antwortet {status}"), None) })
}

/// Eingaben für eine neue Spule
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSpool {
    pub brand: String,
    pub material: String,
    /// Produktname, z.B. „PLA Matte“; leer = Material
    #[serde(default)]
    pub series: String,
    /// #RRGGBB
    pub color: String,
    /// Name wie in Bambu Handy
    #[serde(default)]
    pub name: String,
    pub total_grams: f64,
    pub remaining_grams: f64,
    #[serde(default)]
    pub note: String,
    #[serde(default = "one")]
    pub count: u32,
}
fn one() -> u32 {
    1
}

/// Farbe ins Format des Kontos (`#RRGGBBAA`)
fn cloud_color(hex: &str) -> String {
    let h = hex.trim().trim_start_matches('#').to_uppercase();
    match h.len() {
        6 => format!("#{h}FF"),
        8 => format!("#{h}"),
        _ => "#FFFFFFFF".into(),
    }
}

/// Vorlage wählen: gleiche Marke+Material+Produkt, sonst gleiches Material, sonst irgendeine Spule ohne RFID.
fn pick_template<'a>(hits: &'a [Value], n: &NewSpool) -> Option<&'a Value> {
    let eq = |v: &Value, s: &str| v.as_str().map(|x| x.eq_ignore_ascii_case(s.trim())).unwrap_or(false);
    let series = if n.series.trim().is_empty() { n.material.as_str() } else { n.series.as_str() };
    hits.iter()
        .find(|x| eq(&x["filamentVendor"], &n.brand) && eq(&x["filamentType"], &n.material) && eq(&x["filamentName"], series))
        .or_else(|| hits.iter().find(|x| eq(&x["filamentType"], &n.material) && x["RFID"].as_str().unwrap_or("").is_empty()))
        .or_else(|| hits.iter().find(|x| x["RFID"].as_str().unwrap_or("").is_empty()))
}

/// Inhalt für POST: Vorlage ohne Geräte- und Kennfelder, mit den neuen Werten.
fn new_spool_body(tpl: Option<&Value>, n: &NewSpool) -> Value {
    let same_type = tpl.map(|t| t["filamentType"].as_str().unwrap_or("").eq_ignore_ascii_case(n.material.trim())).unwrap_or(false);
    let mut body = tpl.cloned().unwrap_or_else(|| json!({}));
    if let Some(o) = body.as_object_mut() {
        for k in ["id", "createdAt", "updatedAt", "RFID", "inPrinter", "devId", "amsSn", "amsId", "amsType", "slotId", "trayIdName", "deviceName", "depleted"] {
            o.remove(k);
        }
        if !same_type {
            o.remove("filamentId");
            o.remove("category");
        }
    }
    let c = cloud_color(&n.color);
    let series = if n.series.trim().is_empty() { n.material.trim() } else { n.series.trim() };
    body["filamentVendor"] = json!(n.brand.trim());
    body["filamentType"] = json!(n.material.trim());
    body["filamentName"] = json!(series);
    body["color"] = json!(c);
    body["colors"] = json!([c]);
    body["displayName"] = json!(n.name.trim());
    body["note"] = json!(n.note.trim());
    body["totalNetWeight"] = json!(n.total_grams.max(0.0).round() as i64);
    body["netWeight"] = json!(n.remaining_grams.clamp(0.0, n.total_grams.max(0.0)).round() as i64);
    body["createType"] = json!("manual");
    if !same_type {
        body["category"] = json!(n.material.trim());
    }
    body
}

/// Neue Spule(n) im Konto anlegen.
pub async fn create(app: &AppHandle, n: &NewSpool) -> Result<WriteResult> {
    let b = app.state::<Bambu>();
    ensure_enabled(&b).await?;
    if n.material.trim().is_empty() || n.total_grams <= 0.0 {
        return Err(AppError::Invalid("Material und Gesamtgewicht angeben.".into()));
    }
    let count = n.count.clamp(1, 10);
    let before = super::fetch_all_spools(&b).await?;
    let hits = before["hits"].as_array().cloned().unwrap_or_default();
    let known: std::collections::HashSet<String> = hits.iter().map(|x| x["id"].to_string()).collect();
    let body = new_spool_body(pick_template(&hits, n), n);
    let mut fail = None;
    for _ in 0..count {
        let (status, text) = b.send_raw(Method::POST, LIST, Some(&body)).await?;
        if !(200..300).contains(&status) {
            fail = Some(format!("Konto antwortet {status}: {}", text.chars().take(160).collect::<String>()));
            break;
        }
    }
    // neu angelegte Spulen per erneutem Lesen finden
    let after = super::fetch_all_spools(&b).await?;
    let new: Vec<Value> = after["hits"].as_array().cloned().unwrap_or_default().into_iter().filter(|x| !known.contains(&x["id"].to_string())).collect();
    let lbl = format!("{} {} {}", n.brand.trim(), if n.series.trim().is_empty() { n.material.trim() } else { n.series.trim() }, n.name.trim()).trim().to_string();
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    for x in &new {
        ctx.store
            .log_filament_write(&x["id"].to_string(), &lbl, "anlegen", None, Some(format!("{}", x["netWeight"])), "ok", "neue Spule", Some(body.to_string()))
            .await?;
    }
    if let Some(msg) = fail {
        ctx.store.log_filament_write("", &lbl, "anlegen", None, None, "fehler", &msg, Some(body.to_string())).await?;
        return Ok(res("fehler", format!("{msg} ({} von {count} angelegt)", new.len()), None));
    }
    Ok(if new.len() as u32 >= count {
        res("ok", if count == 1 { format!("{lbl}: im Bambu-Konto angelegt") } else { format!("{count} Spulen {lbl} im Bambu-Konto angelegt") }, None)
    } else {
        res("fehler", format!("Angefragt: {count}, im Konto gefunden: {}", new.len()), None)
    })
}

/// Vorrat verringern: eine Spule im Konto löschen (nicht im Drucker, ohne RFID).
pub async fn stock_remove(app: &AppHandle, id: &str) -> Result<WriteResult> {
    let b = app.state::<Bambu>();
    ensure_enabled(&b).await?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let spool = ctx.store.get_spool(id).await?;
    if spool.in_printer != 0 {
        return Ok(res("fehler", "Die Spule steckt im Drucker – erst herausnehmen.", None));
    }
    if !spool.rfid.is_empty() {
        return Ok(res("rfid", "Bambu-Spulen mit RFID verwaltet das AMS – bitte in Bambu Studio oder Handy löschen.", None));
    }
    let snapshot = cloud_spool(&b, id).await?.map(|v| v.to_string());
    let num: i64 = id.parse().map_err(|_| AppError::Invalid("Ungültige Spulen-ID".into()))?;
    let (status, text) = b.send_raw(Method::DELETE, &format!("{LIST}/batch"), Some(&json!({ "ids": [num] }))).await?;
    let ok = (200..300).contains(&status) && cloud_spool(&b, id).await?.is_none();
    ctx.store
        .log_filament_write(id, &label(&spool), "loeschen", Some(format!("{:.0}", spool.remaining_grams)), None, if ok { "ok" } else { "fehler" },
            if ok { "" } else { &text }, snapshot)
        .await?;
    Ok(if ok { res("ok", format!("Spule {} im Bambu-Konto gelöscht", label(&spool)), None) } else { res("fehler", format!("Konto antwortet {status}"), None) })
}

/// Wert im selben JSON-Typ wie die Vorlage (Zahl oder Text), damit der Server ihn annimmt.
fn like(template: &Value, n: i64) -> Value {
    if template.is_string() { json!(n.to_string()) } else { json!(n) }
}

/// Zeigt das Konto die gewünschte Zuordnung?
fn slot_matches(v: &Value, dev: Option<&str>, ams: i64, slot: i64) -> bool {
    let num = |x: &Value| x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok()));
    match dev {
        Some(d) => v["inPrinter"].as_bool() == Some(true) && v["devId"].as_str() == Some(d) && num(&v["amsId"]) == Some(ams) && num(&v["slotId"]) == Some(slot),
        None => v["inPrinter"].as_bool() != Some(true),
    }
}

/// Spule im Konto einem Drucker/AMS-Slot zuordnen (`printer = None`: Zuordnung lösen).
/// Ändert nur den Eintrag im Konto, nicht den Drucker. Ein späterer AMS-Abgleich von Bambu kann ihn überschreiben.
pub async fn assign_slot(app: &AppHandle, id: &str, printer: Option<&str>, ams: i64, slot: i64) -> Result<WriteResult> {
    let b = app.state::<Bambu>();
    ensure_enabled(&b).await?;
    let (spool, printer_name) = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        let spool = ctx.store.get_spool(id).await?;
        let name = match printer {
            Some(p) => b.printers.read().await.iter().find(|x| x.id == p).map(|x| x.name.clone())
                .ok_or_else(|| AppError::NotFound("Drucker".into()))?,
            None => String::new(),
        };
        (spool, name)
    };
    if spool.source != "konto" {
        return Ok(res("fehler", "Die Spule stammt nicht aus dem Bambu-Konto.", None));
    }
    if !spool.rfid.is_empty() {
        return Ok(res("rfid", "Bambu-Spulen mit RFID ordnet das AMS selbst zu.", None));
    }
    let Some(cloud) = cloud_spool(&b, id).await? else { return Ok(res("fehler", "Die Spule gibt es im Bambu-Konto nicht mehr.", None)) };
    // Vorlage für die Feldtypen: eine Spule, die schon in einem Drucker steckt
    let all = super::fetch_all_spools(&b).await.unwrap_or(Value::Null);
    let tpl = all["hits"].as_array().and_then(|a| a.iter().find(|x| x["inPrinter"].as_bool() == Some(true)).cloned()).unwrap_or(Value::Null);
    let mut body = json!({ "id": cloud["id"], "filamentName": cloud["filamentName"] });
    match printer {
        Some(dev) => {
            body["inPrinter"] = json!(true);
            body["devId"] = json!(dev);
            body["deviceName"] = json!(printer_name);
            body["amsId"] = like(&tpl["amsId"], ams);
            body["slotId"] = like(&tpl["slotId"], slot);
        }
        None => {
            body["inPrinter"] = json!(false);
            body["devId"] = json!("");
        }
    }
    let lbl = label(&spool);
    let target = match printer {
        Some(_) => format!("{printer_name} · {}", if ams >= 254 { "Externe Spule".into() } else { format!("AMS {} · Slot {}", ams + 1, slot + 1) }),
        None => "kein Drucker".into(),
    };
    let old = Some(if spool.in_printer != 0 { format!("{} · {}", spool.printer_name, spool.ams_slot) } else { "kein Drucker".into() });
    let (status, text) = b.send_raw(Method::PUT, LIST, Some(&body)).await?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    if !(200..300).contains(&status) {
        let msg = format!("Konto antwortet {status}: {}", text.chars().take(160).collect::<String>());
        ctx.store.log_filament_write(id, &lbl, "slot", old, Some(target), "fehler", &msg, Some(cloud.to_string())).await?;
        return Ok(res("fehler", msg, None));
    }
    let after = cloud_spool(&b, id).await?.unwrap_or(Value::Null);
    let ok = slot_matches(&after, printer, ams, slot);
    let seen = format!(
        "Konto zeigt: inPrinter={} devId={} amsId={} slotId={}",
        after["inPrinter"], after["devId"], after["amsId"], after["slotId"]
    );
    ctx.store.log_filament_write(id, &lbl, "slot", old, Some(target.clone()), if ok { "ok" } else { "fehler" }, &seen, Some(cloud.to_string())).await?;
    Ok(if ok {
        res("ok", format!("{lbl}: im Konto {target}"), None)
    } else {
        res("fehler", format!("Das Konto hat die Zuordnung nicht übernommen. {seen}"), None)
    })
}

// ---------- Befehle ----------

#[tauri::command]
pub async fn spool_assign_slot(app: AppHandle, id: String, printer: Option<String>, ams: i64, slot: i64) -> Result<WriteResult> {
    let r = assign_slot(&app, &id, printer.as_deref(), ams, slot).await?;
    let _ = super::sync_filament(&app).await;
    Ok(r)
}

#[tauri::command]
pub async fn spool_push_rest(app: AppHandle, id: String, grams: f64, force: bool) -> Result<WriteResult> {
    push_rest(&app, &id, grams, force).await
}

#[tauri::command]
pub async fn spool_stock_add(app: AppHandle, id: String) -> Result<WriteResult> {
    let r = stock_add(&app, &id).await?;
    let _ = super::sync_filament(&app).await;
    Ok(r)
}

#[tauri::command]
pub async fn spool_create(app: AppHandle, spool: NewSpool) -> Result<WriteResult> {
    let r = create(&app, &spool).await?;
    let _ = super::sync_filament(&app).await;
    Ok(r)
}

#[tauri::command]
pub async fn spool_stock_remove(app: AppHandle, id: String) -> Result<WriteResult> {
    let r = stock_remove(&app, &id).await?;
    let _ = super::sync_filament(&app).await;
    Ok(r)
}

#[tauri::command]
pub async fn filament_writes(app: AppHandle) -> Result<Vec<FilamentWrite>> {
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    ctx.store.list_filament_writes(50).await
}

/// Einen Rest-Eintrag rückgängig machen: den alten Wert ins Konto (und in die App) schreiben.
#[tauri::command]
pub async fn filament_write_undo(app: AppHandle, write_id: String) -> Result<WriteResult> {
    let (spool_id, old) = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        let w = ctx.store.list_filament_writes(500).await?.into_iter().find(|w| w.id == write_id)
            .ok_or_else(|| AppError::NotFound("Eintrag".into()))?;
        if w.action != "rest" || w.status != "ok" {
            return Err(AppError::Invalid("Nur gespeicherte Rest-Änderungen lassen sich rückgängig machen.".into()));
        }
        (w.spool_id, w.old_value.and_then(|x| x.parse::<f64>().ok()).ok_or_else(|| AppError::Invalid("Kein alter Wert".into()))?)
    };
    push_rest(&app, &spool_id, old, true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeitstempel_wie_beim_abgleich() {
        assert_eq!(stamp(&json!({"updatedAt": 1794817623})).as_deref(), Some("2026-11-16T08:27:03Z"));
        assert_eq!(stamp(&json!({})), None);
    }

    fn neu(material: &str) -> NewSpool {
        NewSpool { brand: "Generic".into(), material: material.into(), series: String::new(), color: "#f95d73".into(), name: "Rot".into(),
            total_grams: 1000.0, remaining_grams: 1200.0, note: String::new(), count: 1 }
    }

    #[test]
    fn neue_spule_nach_vorlage() {
        let hits = vec![
            json!({"id": 1, "filamentVendor": "Bambu Lab", "filamentType": "PLA", "filamentName": "PLA Matte", "RFID": "AB", "filamentId": "GFA01"}),
            json!({"id": 2, "filamentVendor": "Generic", "filamentType": "PLA", "filamentName": "PLA", "RFID": "", "filamentId": "GFL99",
                   "category": "PLA", "inPrinter": true, "devId": "D", "slotId": "1", "colorType": 2}),
        ];
        let n = neu("PLA");
        let b = new_spool_body(pick_template(&hits, &n), &n);
        assert_eq!(b["filamentId"], json!("GFL99"));
        assert_eq!(b["color"], json!("#F95D73FF"));
        assert_eq!(b["netWeight"], json!(1000)); // nie mehr als das Gesamtgewicht
        assert_eq!(b["displayName"], json!("Rot"));
        assert!(b.get("id").is_none() && b.get("devId").is_none() && b.get("inPrinter").is_none());
        // anderes Material: Profil-ID der Vorlage nicht übernehmen
        let p = neu("PETG");
        let b = new_spool_body(pick_template(&hits, &p), &p);
        assert!(b.get("filamentId").is_none());
        assert_eq!((b["filamentType"].clone(), b["category"].clone()), (json!("PETG"), json!("PETG")));
    }

    #[test]
    fn slot_vergleich_tolerant() {
        let v = json!({"inPrinter": true, "devId": "D1", "amsId": 0, "slotId": "2"});
        assert!(slot_matches(&v, Some("D1"), 0, 2));
        assert!(!slot_matches(&v, Some("D1"), 0, 1));
        assert!(!slot_matches(&v, None, 0, 0));
        assert!(slot_matches(&json!({"inPrinter": false}), None, 0, 0));
        assert_eq!(like(&json!("1"), 3), json!("3"));
        assert_eq!(like(&json!(1), 3), json!(3));
    }

    #[test]
    fn konflikt_nur_bei_fremder_aenderung() {
        let spool = Spool { studio_grams: 650.0, source_updated_at: Some("2026-11-16T08:27:03Z".into()), ..Default::default() };
        // unverändert seit dem Abgleich
        assert!(!conflict(&json!({"updatedAt": 1794817623, "netWeight": 650}), &spool));
        // anderer Zeitstempel, gleicher Rest (z.B. Notiz geändert): kein Konflikt
        assert!(!conflict(&json!({"updatedAt": 1794900000, "netWeight": 650}), &spool));
        // in Handy auf 600 g geändert: Konflikt
        assert!(conflict(&json!({"updatedAt": 1794900000, "netWeight": 600}), &spool));
    }
}
