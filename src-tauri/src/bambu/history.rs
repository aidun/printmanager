//! Druckverlauf aus dem Bambu-Konto (`my/tasks`): lokal speichern, automatisch Aufträgen zuordnen,
//! von Hand nachträglich zuordnen. Nur lesen. Gramm sind Slicer-Werte, die Dauer (Ende − Start) ist echt.

use super::printers::task_result;
use crate::matching::{find_order, normalize, Match};
use crate::usage::{usage_factor, JobOutcome};
use super::Bambu;
use crate::commands::AppState;
use crate::error::Result;
use crate::model::{HistoryEntry, HistorySyncResult, RunFilament};
use serde_json::Value;
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

const PAGE: usize = 50;
/// höchstens so viele Seiten bei einem vollständigen Abgleich (Schutz vor Endlosschleifen)
const MAX_PAGES: usize = 60;

/// Bambu-Status im Druckverlauf → neutrales Ergebnis (2 = fertig, 3 = abgebrochen/Fehler)
fn outcome(status: i64) -> JobOutcome {
    match status {
        2 => JobOutcome::Finished,
        3 => JobOutcome::Failed,
        _ => JobOutcome::Unknown,
    }
}

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
        let (factor, method) = usage_factor(outcome(h.status), h.start_time.as_deref(), h.end_time.as_deref(), h.planned_seconds, progress);
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
    fn status_wird_neutral() {
        assert_eq!(outcome(2), JobOutcome::Finished);
        assert_eq!(outcome(3), JobOutcome::Failed);
        assert_eq!(outcome(1), JobOutcome::Unknown);
    }

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
}
