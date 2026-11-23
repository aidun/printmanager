//! Einheitliche Schicht für externe Anbindungen (docs/PLAN-Anbindungen.md, Stufe A1).
//!
//! Eine **Anbindung** (`Connector`) ist ein externer Dienst oder ein Programm, z.B. das Bambu-Konto oder
//! Google Kontakte. Sie meldet in ihrem `Descriptor`, welche **Fähigkeiten** (`Cap`) sie hat. Der Kern
//! fragt nur nach Fähigkeiten, nie nach Anbietern. Gemeinsam für alle Anbindungen:
//! - `http::HttpGateway`: Drosselung, Zeitlimit, Schreibschutz (Nicht-GET nur auf erlaubten Pfaden und nur mit Schalter)
//! - `secrets::SecretScope`: Geheimnisse nur im Schlüsselbund, Konto `<anbindung>:<name>`
//! - `config::ConnectorsConfig`: an/aus und Schalter je Fähigkeit (Schreiben ab Werk aus)
//! - `registry::Registry`: alle Anbindungen der Plattform, Verbinden/Trennen, Status
//! - `scheduler::Scheduler`: ein Takt für alle lesenden Fähigkeiten
//!
//! Erste Anbindung: Bambu-Konto (`bambu::BambuConnector`, Stufe A3).
#![allow(dead_code)] // Teile des Gerüsts (Verknüpfungen, Kontakte) werden erst ab Stufe A4/K1 genutzt

pub mod commands;
pub mod config;
pub mod contacts;
pub mod http;
pub mod registry;
pub mod scheduler;
pub mod secrets;
pub mod slicer;

#[cfg(test)]
mod tests;

use crate::error::Result;
use crate::watch::{Changed, EVENT};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Art der Anbindung (für Gruppierung in der Oberfläche)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    PrinterCloud,
    Slicer,
    Catalog,
    Contacts,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    MacOs,
    Windows,
    /// nur für Entwicklung und CI
    Linux,
}

impl Platform {
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
        } else if cfg!(target_os = "linux") {
            Some(Self::Linux)
        } else {
            None
        }
    }
}

/// Wie man sich bei der Anbindung anmeldet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthKind {
    /// keine Anmeldung (z.B. lokales Programm)
    None,
    /// Anmeldefenster, Token aus einem Cookie (Bambu)
    WebviewCookie,
    /// OAuth 2.0 mit PKCE über den Systembrowser (Google)
    OAuthPkce,
    /// Benutzername und App-Passwort (CardDAV)
    AppPassword,
    /// Freigabe durch das Betriebssystem (Apple Kontakte am Mac)
    OsPermission,
}

/// Fähigkeit einer Anbindung, getrennt nach Lesen und Schreiben
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cap {
    SpoolsRead,
    SpoolsWrite,
    PrintersRead,
    PrintHistoryRead,
    CatalogRead,
    LinksRead,
    ContactsRead,
    /// Datei in einem Programm öffnen (Slicer); startet nur das Programm, steuert nie einen Drucker
    SlicerOpen,
    /// im Hintergrund slicen, nur zur Kalkulation (Programmstart ohne Oberfläche, nie ein Druck)
    SlicerSlice,
    /// Druckprofile (Presets) für den Slicer lesen
    ProfilesRead,
}

impl Cap {
    /// Verändert die Fähigkeit Daten beim Anbieter? Solche Fähigkeiten sind ab Werk aus.
    pub fn is_write(self) -> bool {
        matches!(self, Cap::SpoolsWrite)
    }

    /// Werkszustand des Schalters: Lesen an; Schreiben und Slicen im Hintergrund aus
    pub fn default_on(self) -> bool {
        !self.is_write() && self != Cap::SlicerSlice
    }
}

/// Statische Beschreibung einer Anbindung
#[derive(Debug)]
pub struct Descriptor {
    /// stabile ID, z.B. "bambu-cloud"; Teil von Schlüsselbund-Konten und Datenbankeinträgen
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub platforms: &'static [Platform],
    pub auth: AuthKind,
    pub caps: &'static [Cap],
    /// Basisadresse für HTTP; leer bei Anbindungen ohne HTTP
    pub base_url: &'static str,
    /// Pfade, auf die nicht-lesende Anfragen gehen dürfen (exakt, relativ zu `base_url`)
    pub write_paths: &'static [&'static str],
    /// Pfade, auf die POST als reine Abfrage geht (z.B. GraphQL); kein Schreibschalter nötig
    pub read_post_paths: &'static [&'static str],
    /// Zeitlimit einer Anfrage in Sekunden (Downloads brauchen mehr)
    pub timeout_secs: u64,
    /// an, solange der Nutzer nichts anderes einstellt (öffentliche Dienste ohne Anmeldung)
    pub default_enabled: bool,
    /// Mindestabstand zwischen zwei HTTP-Anfragen
    pub min_interval_ms: u64,
    /// Abgleich-Takt je lesender Fähigkeit in Sekunden
    pub intervals: &'static [(Cap, u64)],
    /// Namen der Geheimnisse im Schlüsselbund; „Trennen“ löscht sie
    pub secrets: &'static [&'static str],
    /// Anmeldeprüfung alle so viele Sekunden (0 = keine)
    pub heartbeat_secs: u64,
}

impl Descriptor {
    pub fn has(&self, cap: Cap) -> bool {
        self.caps.contains(&cap)
    }

    pub fn interval(&self, cap: Cap) -> Option<u64> {
        self.intervals.iter().find(|(c, _)| *c == cap).map(|(_, s)| *s)
    }

    pub fn runs_here(&self) -> bool {
        Platform::current().is_some_and(|p| self.platforms.contains(&p))
    }
}

/// Zustand einer Anbindung für Oberfläche und Takt
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    /// Anmeldung abgelaufen, neu verbinden nötig
    pub expired: bool,
    /// angezeigter Kontoname, falls bekannt
    pub account: Option<String>,
    pub last_ok_at: Option<String>,
    pub last_error: Option<String>,
    /// Zusatz für die Oberfläche, z.B. „gültig bis 14.02.2027“ oder „Anmeldefenster ist offen …“
    pub detail: Option<String>,
}

/// Ergebnis eines Abgleichs
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncOutcome {
    /// Meldungen für die Oberfläche
    pub messages: Vec<String>,
    /// lokale Daten haben sich geändert; die Oberfläche lädt neu
    pub changed: bool,
}

impl SyncOutcome {
    pub fn changed(messages: Vec<String>) -> Self {
        Self { messages, changed: true }
    }
}

/// Was eine Anbindung zum Arbeiten bekommt: ihren HTTP-Zugang und ihre Geheimnisse
#[derive(Clone)]
pub struct ConnCtx {
    pub http: Option<Arc<http::HttpGateway>>,
    pub secrets: secrets::SecretScope,
}

#[async_trait]
pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &'static Descriptor;

    /// Aktueller Zustand; darf keine Netzanfrage auslösen.
    async fn status(&self, ctx: &ConnCtx) -> Status;

    /// Anmelden bzw. Freigabe holen (passend zu `descriptor().auth`).
    async fn connect(&self, ctx: &ConnCtx) -> Result<()>;

    /// Abmelden: löscht alle Geheimnisse der Anbindung. Verknüpfungen und lokale Daten bleiben.
    async fn disconnect(&self, ctx: &ConnCtx) -> Result<()> {
        ctx.secrets.clear()
    }

    /// Einen Abgleich einer lesenden Fähigkeit ausführen.
    async fn sync(&self, ctx: &ConnCtx, cap: Cap) -> Result<SyncOutcome>;

    /// Kontakte lesen, falls die Anbindung eine Kontaktquelle ist (`Cap::ContactsRead`)
    fn contacts(&self) -> Option<&dyn contacts::ContactSource> {
        None
    }

    /// Slicer-Fähigkeiten, falls die Anbindung ein Slicer ist (`Cap::SlicerOpen`)
    fn slicer(&self) -> Option<&dyn slicer::SlicerAdapter> {
        None
    }

    /// Takt einer Fähigkeit in Sekunden; darf vom Zustand abhängen (z.B. schneller während eines Drucks).
    async fn interval(&self, _ctx: &ConnCtx, cap: Cap) -> Option<u64> {
        self.descriptor().interval(cap)
    }

    /// Anmeldung prüfen (alle `heartbeat_secs`), z.B. um eine abgelaufene Anmeldung zu melden.
    async fn heartbeat(&self, _ctx: &ConnCtx) -> Result<SyncOutcome> {
        Ok(SyncOutcome::default())
    }
}

/// Öffentlicher Dienst ohne Anmeldung (z.B. Web-Vorschau, Printables): immer verbunden, ohne Takt.
pub struct PublicConnector(pub &'static Descriptor);

#[async_trait]
impl Connector for PublicConnector {
    fn descriptor(&self) -> &'static Descriptor {
        self.0
    }
    async fn status(&self, _ctx: &ConnCtx) -> Status {
        Status { connected: true, ..Default::default() }
    }
    async fn connect(&self, _ctx: &ConnCtx) -> Result<()> {
        Ok(())
    }
    async fn sync(&self, _ctx: &ConnCtx, _cap: Cap) -> Result<SyncOutcome> {
        Ok(SyncOutcome::default())
    }
}

/// Grundschritt des gemeinsamen Takts
const STEP: std::time::Duration = std::time::Duration::from_secs(20);

/// Startet den gemeinsamen Takt für alle Anbindungen. Meldungen und Änderungen gehen als `pm:external`
/// an die Oberfläche; Fehler nur, wenn die Anmeldung abgelaufen ist (sonst ins Log).
pub fn start(app: tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let scheduler = scheduler::Scheduler::default();
        loop {
            let reg = app.state::<Arc<registry::Registry>>().inner().clone();
            let tick = scheduler.run_once(&reg, std::time::Instant::now()).await;
            let mut messages = tick.messages;
            for (id, err) in tick.errors {
                let expired = match reg.get(id) {
                    Ok(e) => e.connector.status(&e.ctx).await.expired,
                    Err(_) => false,
                };
                if expired {
                    if !messages.contains(&err) {
                        messages.push(err);
                    }
                } else {
                    eprintln!("{id}: {err}");
                }
            }
            if tick.changed || !messages.is_empty() {
                let _ = app.emit(EVENT, Changed { messages });
            }
            tokio::time::sleep(STEP).await;
        }
    });
}
