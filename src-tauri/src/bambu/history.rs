//! Druckverlauf aus dem Bambu-Konto (`my/tasks`): lokal speichern, automatisch Aufträgen zuordnen,
//! von Hand nachträglich zuordnen. Nur lesen. Gramm sind Slicer-Werte, die Dauer (Ende − Start) ist echt.

use super::printers::{find_order, normalize, task_result, Match};
use super::Bambu;
use crate::commands::AppState;
use crate::error::Result;
use crate::model::{HistoryEntry, HistorySyncResult, RunFilament};
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

const PAGE: usize = 50;
/// Aufheizen, Kalibrieren usw. vor der ersten Schicht – wird bei der Schätzung über die Laufzeit abgezogen
const WARMUP_SECS: f64 = 300.0;

/// Anteil des geplanten Verbrauchs, der tatsächlich gedruckt wurde.
/// Fertig = 1. Abgebrochen: Fortschritt beim Abbruch, falls die App ihn gesehen hat, sonst
/// (Laufzeit − Aufheizen) / geplante Druckzeit. Ohne Angaben bleibt es beim geplanten Wert.
pub fn usage_factor(status: i64, start: Option<&str>, end: Option<&str>, planned: Option<i64>, progress: Option<i64>) -> (f64, &'static str) {
    if status == 2 {
        return (1.0, "fertig");
    }
    if status != 3 {
        return (1.0, "geplant");
    }
    if let Some(p) = progress {
        return ((p as f64 / 100.0).clamp(0.0, 1.0), "fortschritt");
    }
    let t = |x: Option<&str>| x.and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
    match (t(start), t(end), planned) {
        (Some(a), Some(b), Some(p)) if p > 0 && b >= a => {
            let ran = (b - a).num_seconds() as f64;
            (((ran - WARMUP_SECS) / p as f64).clamp(0.0, 1.0), "zeit")
        }
        _ => (1.0, "geplant"),
    }
}

/// Gramm einer Filament-Liste (JSON) mit dem Faktor multiplizieren.
pub fn scale_filaments(json: &str, factor: f64) -> String {
    let mut list: Vec<RunFilament> = serde_json::from_str(json).unwrap_or_default();
    if (factor - 1.0).abs() > f64::EPSILON {
        for f in &mut list {
            f.grams = (f.grams * factor * 100.0).round() / 100.0;
        }
    }
    serde_json::to_string(&list).unwrap_or_else(|_| "[]".into())
}
/// höchstens so viele Seiten bei einem vollständigen Abgleich (Schutz vor Endlosschleifen)
const MAX_PAGES: usize = 60;

fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Einen Eintrag aus `my/tasks` lesen (Felder tolerant).
pub fn parse_task(t: &Value) -> Option<HistoryEntry> {
    let id = id_of(&t["id"])?;
    let s = |k: &str| t[k].as_str().unwrap_or("").trim().to_string();
    let opt = |k: &str| t[k].as_str().filter(|x| !x.is_empty()).map(str::to_string);
    let fils: Vec<RunFilament> = task_result(&serde_json::json!({
        "status": 2, "amsDetailMapping": t["amsDetailMapping"].clone()
    }))
    .map(|r| r.2)
    .unwrap_or_default();
    Some(HistoryEntry {
        id,
        title: s("title"),
        design_title: s("designTitle"),
        design_id: id_of(&t["designId"]).filter(|x| x != "0"),
        device_name: s("deviceName"),
        device_model: s("deviceModel"),
        plate_index: t["plateIndex"].as_i64(),
        status: t["status"].as_i64().unwrap_or(0),
        start_time: opt("startTime"),
        end_time: opt("endTime"),
        planned_seconds: t["costTime"].as_i64(),
        weight_grams: t["weight"].as_f64(),
        filaments: serde_json::to_string(&fils).unwrap_or_else(|_| "[]".into()),
        cover_url: opt("cover").filter(|c| c.starts_with("https://")),
        ..Default::default()
    })
}

/// Seiten holen. Das Konto dokumentiert das Blättern nicht; zuerst `offset`, sonst `after=<letzte ID>`.
/// Ohne `full` nur die erste Seite (neueste Drucke).
async fn fetch(bambu: &Bambu, full: bool) -> Result<(Vec<HistoryEntry>, i64)> {
    let first = bambu.get(&format!("/v1/user-service/my/tasks?limit={PAGE}")).await?;
    let total = first["total"].as_i64().unwrap_or(0);
    let mut all: Vec<HistoryEntry> = first["hits"].as_array().into_iter().flatten().filter_map(parse_task).collect();
    if !full {
        return Ok((all, total));
    }
    let mut seen: HashSet<String> = all.iter().map(|h| h.id.clone()).collect();
    let mut mode = "offset";
    for _ in 1..MAX_PAGES {
        if all.len() as i64 >= total || all.is_empty() {
            break;
        }
        let path = match mode {
            "offset" => format!("/v1/user-service/my/tasks?limit={PAGE}&offset={}", all.len()),
            _ => format!("/v1/user-service/my/tasks?limit={PAGE}&after={}", all.last().map(|h| h.id.as_str()).unwrap_or("")),
        };
        let page: Vec<HistoryEntry> = bambu.get(&path).await?["hits"].as_array().into_iter().flatten().filter_map(parse_task).collect();
        let new: Vec<HistoryEntry> = page.into_iter().filter(|h| seen.insert(h.id.clone())).collect();
        if new.is_empty() {
            if mode == "offset" {
                mode = "after"; // offset wird nicht unterstützt: mit „after“ weiter versuchen
                continue;
            }
            break;
        }
        all.extend(new);
    }
    eprintln!("Druckverlauf: {} von {total} Einträgen geholt (Blättern: {mode})", all.len());
    Ok((all, total))
}

/// Verlauf abgleichen und neue Einträge automatisch offenen Aufträgen zuordnen, sofern der Druck nach
/// dem Anlegen des Auftrags endete.
pub async fn sync(app: &AppHandle, full: bool) -> Result<HistorySyncResult> {
    let bambu = app.state::<Bambu>();
    let (list, total) = fetch(&bambu, full).await?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let added = ctx.store.upsert_history(&list).await?;
    // geschätzten tatsächlichen Verbrauch (neu) berechnen
    for h in ctx.store.list_history().await? {
        let progress = if h.status == 3 {
            match (h.start_time.as_deref(), h.end_time.as_deref()) {
                (Some(a), Some(b)) => ctx.store.progress_for(&normalize(&h.title), &h.device_name, a, b).await?,
                _ => None,
            }
        } else {
            None
        };
        let (factor, method) = usage_factor(h.status, h.start_time.as_deref(), h.end_time.as_deref(), h.planned_seconds, progress);
        if h.used_factor != Some(factor) || h.used_method.as_deref() != Some(method) || h.progress_at_end != progress {
            ctx.store.set_history_usage(&h.id, factor, method, progress).await?;
        }
    }
    let orders = ctx.store.open_order_files().await?;
    let mut matched = 0;
    for h in ctx.store.list_history().await?.into_iter().filter(|h| h.order_id.is_none() && !h.title.is_empty()) {
        let assigned = ctx.store.print_assignment(&normalize(&h.title)).await?;
        let Some(Match { order, by, .. }) = find_order(&orders, &h.title, assigned.as_deref()) else { continue };
        let ended = h.end_time.as_deref().or(h.start_time.as_deref()).unwrap_or("");
        // Drucke vor dem Anlegen des Auftrags gehören nicht dazu (außer von Hand festgelegt)
        if by != "manuell" && ended < order.created_at.as_str() {
            continue;
        }
        if ctx.store.set_history_auto(&h.id, &order.order_id, by).await? {
            matched += 1;
        }
    }
    Ok(HistorySyncResult { fetched: list.len() as i64, added, matched, total })
}

// ---------- Befehle ----------

#[tauri::command]
pub async fn history_list(app: AppHandle) -> Result<Vec<HistoryEntry>> {
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    ctx.store.list_history().await
}

/// Knopf „Aktualisieren“: ganzen Verlauf abgleichen.
#[tauri::command]
pub async fn history_sync(app: AppHandle) -> Result<HistorySyncResult> {
    sync(&app, true).await
}

/// Druck aus dem Verlauf von Hand einem Auftrag zuordnen (`None` = Zuordnung aufheben).
#[tauri::command]
pub async fn history_assign(app: AppHandle, id: String, order_id: Option<String>) -> Result<()> {
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    ctx.store.assign_history(&id, order_id.as_deref().filter(|s| !s.is_empty())).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eintrag_lesen() {
        let t = serde_json::json!({
            "id": 1449602352, "title": "A0004 benchy_plate_1", "designTitle": "", "designId": 0,
            "deviceName": "h2c", "deviceModel": "O1C2", "plateIndex": 1, "status": 2,
            "startTime": "2026-11-16T07:43:27Z", "endTime": "2026-11-16T15:54:21Z", "costTime": 28367, "weight": 94.17,
            "cover": "https://public-cdn.bblmw.com/x.png",
            "amsDetailMapping": [{"filamentType": "PETG", "sourceColor": "161616FF", "weight": 84.05}]
        });
        let h = parse_task(&t).unwrap();
        assert_eq!((h.id.as_str(), h.status, h.planned_seconds, h.design_id), ("1449602352", 2, Some(28367), None));
        assert!(h.filaments.contains("PETG") && h.filaments.contains("84.05"));
        assert_eq!(h.cover_url.as_deref(), Some("https://public-cdn.bblmw.com/x.png"));
        assert!(parse_task(&serde_json::json!({"title": "ohne id"})).is_none());
    }

    #[test]
    fn verbrauch_bei_abbruch_schaetzen() {
        let (s, e) = (Some("2026-11-16T10:00:00Z"), Some("2026-11-16T10:23:00Z"));
        assert_eq!(usage_factor(2, s, e, Some(3600), None), (1.0, "fertig"));
        // nach 23 Min. abgebrochen, geplant 10 h: (1380 − 300) / 36000 = 3 %
        let (f, m) = usage_factor(3, s, e, Some(36000), None);
        assert_eq!(m, "zeit");
        assert!((f - 0.03).abs() < 0.001);
        // live gesehener Fortschritt hat Vorrang
        assert_eq!(usage_factor(3, s, e, Some(36000), Some(40)), (0.4, "fortschritt"));
        // sofort abgebrochen: 0; länger gelaufen als geplant: höchstens 1
        assert_eq!(usage_factor(3, s, Some("2026-11-16T10:00:16Z"), Some(36000), None).0, 0.0);
        assert_eq!(usage_factor(3, s, e, Some(600), None).0, 1.0);
        // ohne Zeiten bleibt es beim geplanten Wert
        assert_eq!(usage_factor(3, None, None, None, None), (1.0, "geplant"));
        let j = scale_filaments(r##"[{"type":"PLA","color":"#DE4343","grams":100.0}]"##, 0.25);
        assert!(j.contains("25.0"), "{j}");
    }
}
