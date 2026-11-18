//! Ein Takt für alle Anbindungen: ruft lesende Fähigkeiten auf, die an, verbunden und fällig sind.
//! Schreibende Fähigkeiten laufen nie im Takt, nur nach Bestätigung durch den Nutzer.

use super::registry::Registry;
use super::Cap;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Ergebnis eines Taktschritts
#[derive(Debug, Default, PartialEq)]
pub struct Tick {
    /// Meldungen für die Oberfläche (Ereignis `pm:external`)
    pub messages: Vec<String>,
    /// Fehler je Anbindung; der Aufrufer entscheidet, ob und wie er sie zeigt
    pub errors: Vec<(&'static str, String)>,
    /// tatsächlich ausgeführte Abgleiche
    pub ran: Vec<(&'static str, Cap)>,
}

pub fn due(last: Option<Instant>, now: Instant, every: Duration) -> bool {
    last.is_none_or(|t| now.saturating_duration_since(t) >= every)
}

#[derive(Default)]
pub struct Scheduler {
    last: Mutex<HashMap<(&'static str, Cap), Instant>>,
}

impl Scheduler {
    pub async fn run_once(&self, reg: &Registry, now: Instant) -> Tick {
        let mut tick = Tick::default();
        for e in reg.entries() {
            let d = e.connector.descriptor();
            for &(cap, secs) in d.intervals {
                if cap.is_write() || !reg.cap_on(d.id, cap).await {
                    continue;
                }
                let last = self.last.lock().await.get(&(d.id, cap)).copied();
                if !due(last, now, Duration::from_secs(secs)) {
                    continue;
                }
                if !e.connector.status(&e.ctx).await.connected {
                    continue;
                }
                // vor dem Aufruf merken, damit ein Fehler nicht jeden Takt neu anfragt
                self.last.lock().await.insert((d.id, cap), now);
                tick.ran.push((d.id, cap));
                match e.connector.sync(&e.ctx, cap).await {
                    Ok(m) => tick.messages.extend(m),
                    Err(err) => tick.errors.push((d.id, err.to_string())),
                }
            }
        }
        tick
    }

    /// Nächster Abgleich soll sofort laufen (z.B. nach „Jetzt abgleichen“ oder neuer Anmeldung).
    pub async fn reset(&self, id: &str) {
        self.last.lock().await.retain(|(c, _), _| *c != id);
    }
}
