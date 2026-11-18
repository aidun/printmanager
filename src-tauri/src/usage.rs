//! Tatsächlicher Verbrauch eines Drucks – unabhängig vom Anbieter.
//! Bei Abbrüchen schätzt die App den gedruckten Anteil aus dem Fortschritt oder der Laufzeit.

use crate::model::RunFilament;

/// Ergebnis eines Druckauftrags, wie es ein Drucker-Dienst meldet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobOutcome {
    Finished,
    Failed,
    /// läuft noch oder unbekannt
    Unknown,
}

/// Aufheizen, Kalibrieren usw. vor der ersten Schicht – wird bei der Schätzung über die Laufzeit abgezogen
const WARMUP_SECS: f64 = 300.0;

/// Anteil des geplanten Verbrauchs, der tatsächlich gedruckt wurde.
/// Fertig = 1. Abgebrochen: Fortschritt beim Abbruch, falls die App ihn gesehen hat, sonst
/// (Laufzeit − Aufheizen) / geplante Druckzeit. Ohne Angaben bleibt es beim geplanten Wert.
pub fn usage_factor(outcome: JobOutcome, start: Option<&str>, end: Option<&str>, planned: Option<i64>, progress: Option<i64>) -> (f64, &'static str) {
    match outcome {
        JobOutcome::Finished => return (1.0, "fertig"),
        JobOutcome::Unknown => return (1.0, "geplant"),
        JobOutcome::Failed => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbrauch_bei_abbruch_schaetzen() {
        let (s, e) = (Some("2026-11-16T10:00:00Z"), Some("2026-11-16T10:23:00Z"));
        assert_eq!(usage_factor(JobOutcome::Finished, s, e, Some(3600), None), (1.0, "fertig"));
        // nach 23 Min. abgebrochen, geplant 10 h: (1380 − 300) / 36000 = 3 %
        let (f, m) = usage_factor(JobOutcome::Failed, s, e, Some(36000), None);
        assert_eq!(m, "zeit");
        assert!((f - 0.03).abs() < 0.001);
        // live gesehener Fortschritt hat Vorrang
        assert_eq!(usage_factor(JobOutcome::Failed, s, e, Some(36000), Some(40)), (0.4, "fortschritt"));
        // sofort abgebrochen: 0; länger gelaufen als geplant: höchstens 1
        assert_eq!(usage_factor(JobOutcome::Failed, s, Some("2026-11-16T10:00:16Z"), Some(36000), None).0, 0.0);
        assert_eq!(usage_factor(JobOutcome::Failed, s, e, Some(600), None).0, 1.0);
        // ohne Zeiten bleibt es beim geplanten Wert
        assert_eq!(usage_factor(JobOutcome::Failed, None, None, None, None), (1.0, "geplant"));
        let j = scale_filaments(r##"[{"type":"PLA","color":"#DE4343","grams":100.0}]"##, 0.25);
        assert!(j.contains("25.0"), "{j}");
    }
}
