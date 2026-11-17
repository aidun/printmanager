//! Kalkulation: Werte für Kosten und Verkaufspreis (Datei `pricing.json` im Konfigurationsordner).
//! Gerechnet wird in der Oberfläche (`src/pricing.ts`), damit Schätzung und tatsächliche Werte sofort umschalten.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Pricing {
    /// Filamentpreis je Material in €/kg; `default` für alles andere
    pub filament_eur_per_kg: BTreeMap<String, f64>,
    /// mittlere Leistungsaufnahme des Druckers in Watt
    pub power_watts: f64,
    pub power_eur_per_kwh: f64,
    /// Verschleiß und Abschreibung des Druckers je Druckstunde
    pub machine_eur_per_hour: f64,
    /// Aufschlag für Fehldrucke und Ausschuss in Prozent
    pub failure_pct: f64,
    /// Pauschale für Vorbereitung, Nacharbeit, Verpackung je Auftrag
    pub handling_eur: f64,
    /// Verkaufspreis = Kosten × Faktor (ohne Pauschale), danach Pauschale
    pub markup_factor: f64,
    pub min_price_eur: f64,
    /// auf diesen Betrag aufrunden (z.B. 0,50 €)
    pub round_to_eur: f64,
}

impl Default for Pricing {
    fn default() -> Self {
        let filament = [("default", 25.0), ("PLA", 20.0), ("PETG", 22.0), ("TPU", 32.0), ("ASA", 28.0), ("ABS", 22.0), ("PLA-CF", 35.0)]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Self {
            filament_eur_per_kg: filament,
            power_watts: 150.0,
            power_eur_per_kwh: 0.35,
            machine_eur_per_hour: 0.40,
            failure_pct: 10.0,
            handling_eur: 2.0,
            markup_factor: 2.5,
            min_price_eur: 5.0,
            round_to_eur: 0.5,
        }
    }
}

fn file(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_config_dir().map_err(|e| crate::error::AppError::Invalid(e.to_string()))?.join("pricing.json"))
}

pub fn load(path: &Path) -> Pricing {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
pub fn pricing_get(app: AppHandle) -> Result<Pricing> {
    Ok(load(&file(&app)?))
}

#[tauri::command]
pub fn pricing_save(app: AppHandle, pricing: Pricing) -> Result<()> {
    let bad = |x: f64| !x.is_finite() || x < 0.0;
    if [pricing.power_watts, pricing.power_eur_per_kwh, pricing.machine_eur_per_hour, pricing.failure_pct, pricing.handling_eur,
        pricing.markup_factor, pricing.min_price_eur, pricing.round_to_eur]
        .into_iter()
        .any(bad)
        || pricing.filament_eur_per_kg.values().any(|v| bad(*v))
    {
        return Err(crate::error::AppError::Invalid("Bitte nur Zahlen ab 0 eintragen.".into()));
    }
    let path = file(&app)?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(&pricing).unwrap_or_default())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standardwerte_und_teilweise_datei() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("pricing.json");
        assert_eq!(load(&f), Pricing::default());
        std::fs::write(&f, r#"{"markupFactor": 3.0}"#).unwrap();
        let p = load(&f);
        assert_eq!((p.markup_factor, p.power_watts), (3.0, 150.0), "fehlende Felder bekommen Standardwerte");
        assert_eq!(p.filament_eur_per_kg.get("PLA"), Some(&20.0));
    }
}
