//! Drucker über das Bambu-Konto: Liste, Status, laufender Druck und Zuordnung zum Auftrag.
//! Nur lesen. Die App startet, pausiert oder stoppt keinen Druck.
//! Fertige Drucke kommen aus dem Druckverlauf (`my/tasks`) und werden als Vorschlag an den Druck im Auftrag gehängt.

use super::Bambu;
use crate::commands::AppState;
use crate::error::Result;
use crate::matching::{find_order, normalize, Match};
use crate::model::RunFilament;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};

const BIND: &str = "/v1/iot-service/api/user/bind";
const PRINT: &str = "/v1/iot-service/api/user/print";
const TASKS: &str = "/v1/user-service/my/tasks?limit=20";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterView {
    pub id: String,
    pub name: String,
    pub model: String,
    pub online: bool,
    /// offline | idle | printing | paused | finished | failed
    pub state: String,
    /// Status-Text vom Drucker, z.B. RUNNING
    pub raw_status: String,
    pub task_name: Option<String>,
    pub progress: Option<i64>,
    pub remaining_seconds: Option<i64>,
    pub order_id: Option<String>,
    pub order_number: Option<i64>,
    pub order_title: Option<String>,
    /// Wie der Auftrag gefunden wurde: manuell | nummer | datei | titel
    pub matched_by: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintersResult {
    pub enabled: bool,
    pub connected: bool,
    pub printers: Vec<PrinterView>,
    pub updated_at: Option<String>,
    pub error: Option<String>,
}

/// Zustand aus online und dem Status-Text des Kontos.
pub fn state(online: bool, status: &str) -> &'static str {
    if !online {
        return "offline";
    }
    match status.to_uppercase().as_str() {
        "RUNNING" | "PREPARE" | "SLICING" | "PRINTING" => "printing",
        "PAUSE" | "PAUSED" => "paused",
        "SUCCESS" | "FINISH" | "FINISHED" => "finished",
        "FAILED" => "failed",
        _ => "idle",
    }
}

pub fn active(state: &str) -> bool {
    state == "printing" || state == "paused"
}

fn num(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_f64().map(|f| f.round() as i64))
        .or_else(|| v.as_str().and_then(|s| s.trim().trim_end_matches('%').trim().parse::<f64>().ok()).map(|f| f.round() as i64))
}

/// Fortschritt und Restzeit aus einem Eintrag von `user/print` (Felder tolerant, Namen laut Community).
fn progress_of(d: &Value) -> (Option<i64>, Option<i64>) {
    let progress = ["progress", "mc_percent", "task_progress"].iter().find_map(|k| num(&d[*k])).map(|p| p.clamp(0, 100));
    let remaining = num(&d["mc_remaining_time"])
        .or_else(|| num(&d["remain_time"]))
        .map(|m| m * 60)
        .or_else(|| {
            let total = num(&d["prediction"])?;
            let p = progress?;
            Some(total * (100 - p) / 100)
        });
    (progress, remaining)
}

/// Echte Werte aus einem Eintrag des Druckverlaufs: Ergebnis, Dauer, Gramm pro Slot.
pub fn task_result(t: &Value) -> Option<(&'static str, Option<i64>, Vec<RunFilament>)> {
    let status = match num(&t["status"])? {
        2 => "ok",
        3 => "fehldruck",
        _ => return None,
    };
    let time = |k: &str| t[k].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
    let seconds = match (time("startTime"), time("endTime")) {
        (Some(a), Some(b)) if b > a => Some((b - a).num_seconds()),
        _ => None,
    };
    let fils = t["amsDetailMapping"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| {
                    let grams = m["weight"].as_f64()?;
                    let c = m["sourceColor"].as_str().unwrap_or("");
                    Some(RunFilament {
                        kind: m["filamentType"].as_str().unwrap_or("").to_string(),
                        color: if c.len() >= 6 { format!("#{}", c[..6].to_uppercase()) } else { String::new() },
                        grams: (grams * 100.0).round() / 100.0,
                        spool_id: None,
                        slot: None,
                    })
                })
                .filter(|f| f.grams > 0.0)
                .collect()
        })
        .unwrap_or_default();
    Some((status, seconds, fils))
}

/// Konto abfragen: Drucker, ggf. laufende Drucke, Zuordnung zu Aufträgen. Gibt Meldungen bei Zustandswechseln zurück.
pub async fn poll(app: &AppHandle) -> Result<Vec<String>> {
    let bambu = app.state::<Bambu>();
    let bind = bambu.get(BIND).await?;
    let devices = bind["devices"].as_array().cloned().unwrap_or_default();
    let prev = bambu.printers.read().await.clone();
    let was_active = prev.iter().any(|p| active(&p.state));
    let now_active = devices.iter().any(|d| active(state(d["online"].as_bool().unwrap_or(false), d["print_status"].as_str().unwrap_or(""))));

    let mut live: HashMap<String, Value> = HashMap::new();
    if was_active || now_active {
        if let Ok(p) = bambu.get(PRINT).await {
            for d in p["devices"].as_array().into_iter().flatten() {
                if let Some(id) = d["dev_id"].as_str() {
                    live.insert(id.to_string(), d.clone());
                }
            }
        }
    }

    let orders = {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.store.open_order_files().await?
    };
    let ts = crate::store::now();
    let mut list = Vec::new();
    for d in &devices {
        let id = d["dev_id"].as_str().unwrap_or_default().to_string();
        let online = d["online"].as_bool().unwrap_or(false);
        let l = live.get(&id);
        let raw = l
            .and_then(|l| l["task_status"].as_str())
            .filter(|s| !s.is_empty())
            .or(d["print_status"].as_str())
            .unwrap_or("")
            .to_string();
        let st = state(online, &raw);
        let task_name = l.and_then(|l| l["task_name"].as_str()).filter(|s| !s.is_empty()).map(str::to_string);
        let (progress, remaining_seconds) = l.map(progress_of).unwrap_or((None, None));
        let assigned = match task_name.as_deref() {
            Some(t) => {
                let state = app.state::<AppState>();
                let ctx = state.ctx.read().await;
                ctx.store.print_assignment(&normalize(t)).await?
            }
            None => None,
        };
        // Fortschritt merken: Bricht der Druck ab, lässt sich der tatsächliche Verbrauch daraus schätzen
        if let (Some(t), Some(p)) = (task_name.as_deref(), progress) {
            if active(st) {
                let state = app.state::<AppState>();
                let ctx = state.ctx.read().await;
                let dev = d["name"].as_str().or(d["dev_name"].as_str()).unwrap_or("");
                let _ = ctx.store.save_progress(&normalize(t), dev, p).await;
            }
        }
        let m = task_name.as_deref().and_then(|t| find_order(&orders, t, assigned.as_deref()));
        let order = m.as_ref().map(|m| m.order);
        list.push(PrinterView {
            name: d["name"].as_str().or(d["dev_name"].as_str()).unwrap_or("Drucker").to_string(),
            model: d["dev_product_name"].as_str().unwrap_or("").to_string(),
            online,
            state: st.into(),
            raw_status: raw,
            task_name,
            progress: if active(st) { progress } else { None },
            remaining_seconds: if active(st) { remaining_seconds } else { None },
            order_id: order.map(|o| o.order_id.clone()),
            order_number: order.map(|o| o.number),
            order_title: order.map(|o| o.title.clone()),
            matched_by: m.as_ref().map(|m| m.by.to_string()),
            updated_at: ts.clone(),
            id,
        });
    }

    // Wechsel erkennen: Druck beginnt / endet
    let mut messages = Vec::new();
    let mut finished = false;
    for p in &list {
        let before = prev.iter().find(|x| x.id == p.id);
        let was = before.map(|b| b.state.as_str()).unwrap_or("");
        if active(&p.state) && !active(was) && !was.is_empty() {
            let what = p.order_number.map(|n| format!(" (#{n})")).unwrap_or_default();
            messages.push(format!("{} druckt: {}{what}", p.name, p.task_name.clone().unwrap_or_default()));
        }
        if active(was) && !active(&p.state) {
            finished = true;
        }
    }
    *bambu.printers.write().await = list;
    *bambu.printers_at.write().await = Some(ts);
    if finished {
        messages.extend(match_finished(app).await.unwrap_or_default());
    }
    Ok(messages)
}

/// Druckverlauf mit offenen Drucken in Aufträgen abgleichen. Ein fertiger Druck im Verlauf, der zu einem
/// Druck ohne Rückmeldung passt und nach dessen Anlegen endete, wird als Vorschlag eingetragen.
/// Läuft auch nach Programmstart, damit Drucke erkannt werden, die geendet haben, während die App zu war.
pub async fn match_finished(app: &AppHandle) -> Result<Vec<String>> {
    let bambu = app.state::<Bambu>();
    let tasks = bambu.get(TASKS).await?;
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let orders = ctx.store.open_order_files().await?;
    let mut messages = Vec::new();
    for t in tasks["hits"].as_array().into_iter().flatten() {
        let Some((status, seconds, mut fils)) = task_result(t) else { continue };
        let title = t["title"].as_str().unwrap_or("");
        let printer = t["deviceName"].as_str().unwrap_or("Drucker");
        let end = t["endTime"].as_str().unwrap_or("");
        if status == "fehldruck" {
            // abgebrochen: geschätzter tatsächlicher Verbrauch statt des vollen geplanten Werts
            let start = t["startTime"].as_str();
            let progress = match start {
                Some(a) => ctx.store.progress_for(&normalize(title), printer, a, end).await?,
                None => None,
            };
            let (factor, _) = crate::usage::usage_factor(crate::usage::JobOutcome::Failed, start, Some(end), t["costTime"].as_i64(), progress);
            for f in &mut fils {
                f.grams = (f.grams * factor * 100.0).round() / 100.0;
            }
        }
        let assigned = ctx.store.print_assignment(&normalize(title)).await?;
        let Some(Match { order, file, .. }) = find_order(&orders, title, assigned.as_deref()) else { continue };
        let id = match &t["id"] {
            Value::Number(n) => n.to_string(),
            Value::String(x) if !x.is_empty() => x.clone(),
            _ => continue,
        };
        let task = crate::store::PrinterTask { id: &id, printer, status, ended_at: end, actual_seconds: seconds, actual: &fils };
        // nur Drucke im Auftrag, die vor dem Ende angelegt wurden; jeder Verlaufseintrag nur einmal
        if ctx.store.record_printer_result(&order.order_id, file, &task).await?.is_some() {
            let label = if status == "ok" { "fertig" } else { "abgebrochen" };
            messages.push(format!("#{} {}: Druck auf {printer} {label} – Ergebnis im Auftrag bestätigen", order.number, order.title));
        }
    }
    Ok(messages)
}

#[tauri::command]
pub async fn printers_list(bambu: State<'_, Bambu>) -> Result<PrintersResult> {
    let st = bambu.status().await;
    Ok(PrintersResult {
        enabled: st.config.printers,
        connected: st.connected,
        printers: bambu.printers.read().await.clone(),
        updated_at: bambu.printers_at.read().await.clone(),
        error: st.last_error,
    })
}

/// Druckauftrag von Hand einem Auftrag zuordnen (`order_id`) oder wieder automatisch zuordnen lassen (`None`).
/// Gilt für den laufenden Druck und für die Rückmeldung nach dem Druck.
#[tauri::command]
pub async fn printer_assign(app: AppHandle, task_name: String, order_id: Option<String>) -> Result<PrintersResult> {
    {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.store.set_print_assignment(&normalize(&task_name), order_id.as_deref().filter(|s| !s.is_empty())).await?;
    }
    poll(&app).await?;
    printers_list(app.state::<Bambu>()).await
}

/// Knopf „Aktualisieren“ auf der Drucker-Seite.
#[tauri::command]
pub async fn printers_refresh(app: AppHandle) -> Result<PrintersResult> {
    poll(&app).await?;
    let _ = match_finished(&app).await;
    printers_list(app.state::<Bambu>()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zustand() {
        assert_eq!(state(false, "RUNNING"), "offline");
        assert_eq!(state(true, "RUNNING"), "printing");
        assert_eq!(state(true, "pause"), "paused");
        assert_eq!(state(true, "SUCCESS"), "finished");
        assert_eq!(state(true, ""), "idle");
    }





    #[test]
    fn fortschritt_und_verlauf() {
        let (p, r) = progress_of(&serde_json::json!({"progress": "45%", "mc_remaining_time": 80}));
        assert_eq!((p, r), (Some(45), Some(4800)));
        let (p, r) = progress_of(&serde_json::json!({"progress": 50, "prediction": 3600}));
        assert_eq!((p, r), (Some(50), Some(1800)));

        let t = serde_json::json!({"status": 2, "startTime": "2026-11-16T07:43:27Z", "endTime": "2026-11-16T15:54:21Z",
            "amsDetailMapping": [{"filamentType": "PETG", "sourceColor": "161616FF", "weight": 84.05},
                                 {"filamentType": "PLA", "sourceColor": "DE4343FF", "weight": 0.0}]});
        let (st, secs, fils) = task_result(&t).unwrap();
        assert_eq!((st, secs), ("ok", Some(29454)));
        assert_eq!(fils, vec![RunFilament { kind: "PETG".into(), color: "#161616".into(), grams: 84.05, spool_id: None, slot: None }]);
        assert_eq!(task_result(&serde_json::json!({"status": 3})).unwrap().0, "fehldruck");
        assert!(task_result(&serde_json::json!({"status": 4})).is_none());
    }
}
