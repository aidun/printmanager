//! Spulen aus dem Filament-Manager des Bambu-Kontos lesen (Format des Kontos).

use crate::error::{AppError, Result};
use crate::model::StudioSpool;

/// Spulen aus dem Filament-Manager des Bambu-Kontos (`/v1/design-user-service/my/filament/v2`).
/// Felder laut Test vom 16.11.2026; unbekannte oder fehlende Felder werden toleriert.
pub fn parse_cloud(v: &serde_json::Value) -> Result<Vec<StudioSpool>> {
    let list = v["hits"].as_array().ok_or_else(|| AppError::Invalid("Filament aus dem Konto: Liste „hits“ fehlt".into()))?;
    let s = |x: &serde_json::Value| x.as_str().unwrap_or("").trim().to_string();
    let f = |x: &serde_json::Value| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0);
    Ok(list
        .iter()
        .filter_map(|sp| {
            let id = match &sp["id"] {
                serde_json::Value::String(s) if !s.is_empty() => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => return None,
            };
            let mut c = crate::filament::color(&s(&sp["color"]));
            if c.is_empty() {
                c = sp["colors"].as_array().and_then(|a| a.first()).map(|x| crate::filament::color(&s(x))).unwrap_or_default();
            }
            let in_printer = sp["inPrinter"].as_bool().unwrap_or(false);
            let ams_slot = if in_printer { slot_label(&sp["amsId"], &sp["slotId"]) } else { String::new() };
            let updated = sp["updatedAt"]
                .as_i64()
                .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            Some(StudioSpool {
                id,
                brand: s(&sp["filamentVendor"]),
                series: s(&sp["filamentName"]),
                material: s(&sp["filamentType"]),
                color: c,
                color_name: s(&sp["displayName"]),
                initial_grams: f(&sp["totalNetWeight"]),
                net_grams: f(&sp["netWeight"]),
                status: match &sp["status"] {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(x) => x.clone(),
                    x => x.to_string(),
                },
                source: "konto".into(),
                source_updated_at: updated,
                in_printer,
                printer_name: if in_printer { s(&sp["deviceName"]) } else { String::new() },
                ams_slot,
                depleted: sp["depleted"].as_bool().unwrap_or(false),
                rfid: s(&sp["RFID"]),
            })
        })
        .collect())
}

/// „AMS 1 · Slot 2“ aus `amsId` (0-basiert) und `slotId` (0-basiert, oft als Text).
/// AMS HT hat IDs ab 128, 254/255 sind die externen Halter.
fn slot_label(ams: &serde_json::Value, slot: &serde_json::Value) -> String {
    let ams = ams.as_i64().or_else(|| ams.as_str().and_then(|s| s.parse().ok()));
    let slot = slot.as_i64().or_else(|| slot.as_str().and_then(|s| s.parse().ok()));
    match (ams, slot) {
        (Some(254 | 255), _) => "Externe Spule".into(),
        (Some(a), _) if a >= 128 => format!("AMS HT {}", a - 127),
        (Some(a), Some(s)) => format!("AMS {} · Slot {}", a + 1, s + 1),
        (Some(a), None) => format!("AMS {}", a + 1),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spulen_aus_dem_konto() {
        let v: serde_json::Value = serde_json::from_str(r##"{"hits":[
            {"id":19071734,"filamentVendor":"Bambu Lab","filamentName":"PLA Matte","filamentType":"PLA",
             "color":"#DE4343FF","colors":["#DE4343FF"],"netWeight":650,"totalNetWeight":1000,"inPrinter":true,
             "deviceName":"h2c","amsId":0,"slotId":"1","depleted":false,"status":0,"updatedAt":1794817623,"displayName":null},
            {"id":10168266,"filamentType":"PETG","color":"","colors":["#161616FF"],"netWeight":0,"totalNetWeight":1000,
             "inPrinter":false,"depleted":true},
            {"id":null}
        ]}"##).unwrap();
        let l = parse_cloud(&v).unwrap();
        assert_eq!(l.len(), 2);
        let a = &l[0];
        assert_eq!((a.id.as_str(), a.color.as_str(), a.net_grams, a.initial_grams), ("19071734", "#DE4343", 650.0, 1000.0));
        assert_eq!((a.printer_name.as_str(), a.ams_slot.as_str(), a.source.as_str()), ("h2c", "AMS 1 · Slot 2", "konto"));
        assert!(a.source_updated_at.as_deref().unwrap().starts_with("2026-11-"));
        assert_eq!((l[1].color.as_str(), l[1].depleted, l[1].ams_slot.as_str()), ("#161616", true, ""));
        assert_eq!(slot_label(&serde_json::json!(128), &serde_json::json!(0)), "AMS HT 1");
        assert_eq!(slot_label(&serde_json::json!(255), &serde_json::json!(0)), "Externe Spule");
        assert!(parse_cloud(&serde_json::json!({})).is_err());
    }
}
