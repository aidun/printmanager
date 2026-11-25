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
    /// lokale Daten haben sich geändert
    pub changed: bool,
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
    heartbeat: Mutex<HashMap<&'static str, Instant>>,
}

impl Scheduler {
    pub async fn run_once(&self, reg: &Registry, now: Instant) -> Tick {
        let mut tick = Tick::default();
        for e in reg.entries() {
            let d = e.connector.descriptor();
            if !reg.config().await.is_enabled(d) {
                continue;
            }
            if d.heartbeat_secs > 0 {
                let last = self.heartbeat.lock().await.get(d.id).copied();
                if due(last, now, Duration::from_secs(d.heartbeat_secs)) && e.connector.status(&e.ctx).await.connected {
                    self.heartbeat.lock().await.insert(d.id, now);
                    match e.connector.heartbeat(&e.ctx).await {
                        Ok(o) => {
                            tick.changed |= o.changed;
                            tick.messages.extend(o.messages);
                        }
                        Err(err) => tick.errors.push((d.id, err.to_string())),
                    }
                }
            }
            for &(cap, _) in d.intervals {
                if cap.is_write() || !reg.cap_on(d.id, cap).await {
                    continue;
                }
                let Some(secs) = e.connector.interval(&e.ctx, cap).await else { continue };
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
                    Ok(o) => {
                        tick.changed |= o.changed;
                        tick.messages.extend(o.messages);
                    }
                    Err(err) => tick.errors.push((d.id, err.to_string())),
                }
            }
        }
        tick
    }

}
