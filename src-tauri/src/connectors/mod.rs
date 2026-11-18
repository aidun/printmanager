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
//! Noch ohne Anbindungen; Bambu und Co. ziehen ab Stufe A3 hierher um.
#![allow(dead_code)] // Gerüst aus Stufe A1, wird ab Stufe A3 genutzt

pub mod config;
pub mod http;
pub mod registry;
pub mod scheduler;
pub mod secrets;

#[cfg(test)]
mod tests;

use crate::error::Result;
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
}

impl Platform {
    pub fn current() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Self::MacOs)
        } else if cfg!(target_os = "windows") {
            Some(Self::Windows)
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
}

impl Cap {
    /// Verändert die Fähigkeit Daten beim Anbieter? Solche Fähigkeiten sind ab Werk aus.
    pub fn is_write(self) -> bool {
        matches!(self, Cap::SpoolsWrite)
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
    /// Mindestabstand zwischen zwei HTTP-Anfragen
    pub min_interval_ms: u64,
    /// Abgleich-Takt je lesender Fähigkeit in Sekunden
    pub intervals: &'static [(Cap, u64)],
    /// Namen der Geheimnisse im Schlüsselbund; „Trennen“ löscht sie
    pub secrets: &'static [&'static str],
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

    /// Einen Abgleich einer lesenden Fähigkeit ausführen; liefert Meldungen für die Oberfläche.
    async fn sync(&self, ctx: &ConnCtx, cap: Cap) -> Result<Vec<String>>;
}
