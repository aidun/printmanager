//! Alle Anbindungen der aktuellen Plattform mit ihren Einstellungen, ihrem HTTP-Zugang und ihren Geheimnissen.

use super::config::ConnectorsConfig;
use super::http::HttpGateway;
use super::secrets::{SecretBackend, SecretScope};
use super::{AuthKind, Cap, ConnCtx, Connector, Kind, Status};
use crate::error::{AppError, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Entry {
    pub connector: Arc<dyn Connector>,
    pub ctx: ConnCtx,
}

/// Für die Oberfläche: eine Anbindung mit Schaltern und Zustand
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub auth: AuthKind,
    pub enabled: bool,
    pub caps: Vec<CapInfo>,
    pub status: Status,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapInfo {
    pub cap: Cap,
    pub write: bool,
    /// wirksam (Anbindung an und Schalter an)
    pub on: bool,
    /// Stellung des Schalters, auch wenn die Anbindung gerade aus ist
    pub value: bool,
}

pub struct Registry {
    dir: PathBuf,
    config: RwLock<ConnectorsConfig>,
    entries: Vec<Entry>,
}

impl Registry {
    /// `dir`: Konfigurationsordner mit `connectors.json`. Anbindungen anderer Plattformen werden übergangen.
    pub fn new(dir: PathBuf, backend: Arc<dyn SecretBackend>, connectors: Vec<Arc<dyn Connector>>) -> Result<Self> {
        let config = ConnectorsConfig::load(&dir);
        let mut entries: Vec<Entry> = Vec::new();
        for c in connectors {
            let d = c.descriptor();
            if !d.runs_here() {
                continue;
            }
            if entries.iter().any(|e| e.connector.descriptor().id == d.id) {
                return Err(AppError::Invalid(format!("Anbindung „{}“ doppelt registriert", d.id)));
            }
            let http = (!d.base_url.is_empty() || d.caps.contains(&Cap::LinksRead))
                .then(|| Arc::new(HttpGateway::new(d)));
            let ctx = ConnCtx { http, secrets: SecretScope::new(backend.clone(), d.id, d.secrets) };
            entries.push(Entry { connector: c, ctx });
        }
        let r = Self { dir, config: RwLock::new(config.clone()), entries };
        r.apply_write_flags(&config);
        Ok(r)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn get(&self, id: &str) -> Result<&Entry> {
        self.entries
            .iter()
            .find(|e| e.connector.descriptor().id == id)
            .ok_or_else(|| AppError::NotFound(format!("Anbindung „{id}“")))
    }

    pub async fn config(&self) -> ConnectorsConfig {
        self.config.read().await.clone()
    }

    pub async fn cap_on(&self, id: &str, cap: Cap) -> bool {
        match self.get(id) {
            Ok(e) => self.config.read().await.cap_on(e.connector.descriptor(), cap),
            Err(_) => false,
        }
    }

    /// Schreibschalter in die HTTP-Zugänge übernehmen: Schreiben nur, wenn eine schreibende Fähigkeit an ist.
    fn apply_write_flags(&self, cfg: &ConnectorsConfig) {
        for e in &self.entries {
            let d = e.connector.descriptor();
            if let Some(h) = &e.ctx.http {
                h.set_write_enabled(d.caps.iter().any(|c| c.is_write() && cfg.cap_on(d, *c)));
            }
        }
    }

    async fn update(&self, change: impl FnOnce(&mut ConnectorsConfig)) -> Result<()> {
        let mut cfg = self.config.write().await;
        let mut next = cfg.clone();
        change(&mut next);
        next.save(&self.dir)?;
        self.apply_write_flags(&next);
        *cfg = next;
        Ok(())
    }

    pub async fn set_enabled(&self, id: &str, on: bool) -> Result<()> {
        self.get(id)?;
        self.update(|c| c.entry(id).enabled = Some(on)).await
    }

    pub async fn set_cap(&self, id: &str, cap: Cap, on: bool) -> Result<()> {
        if !self.get(id)?.connector.descriptor().has(cap) {
            return Err(AppError::Invalid(format!("Anbindung „{id}“ hat diese Fähigkeit nicht")));
        }
        self.update(|c| {
            c.entry(id).caps.insert(cap, on);
        })
        .await
    }

    pub async fn set_options(&self, id: &str, options: serde_json::Value) -> Result<()> {
        self.get(id)?;
        self.update(|c| c.entry(id).options = options).await
    }

    /// Verbinden bzw. Freigabe holen; danach ist die Anbindung eingeschaltet.
    pub async fn connect(&self, id: &str) -> Result<()> {
        let e = self.get(id)?;
        e.connector.connect(&e.ctx).await?;
        self.set_enabled(id, true).await
    }

    /// Trennen: Geheimnisse löschen und die Anbindung ausschalten. Lokale Daten und Verknüpfungen bleiben.
    pub async fn disconnect(&self, id: &str) -> Result<()> {
        let e = self.get(id)?;
        e.connector.disconnect(&e.ctx).await?;
        self.set_enabled(id, false).await
    }

    /// Der eingeschaltete Slicer (erster mit `Cap::SlicerOpen`), z.B. Bambu Studio
    pub async fn slicer(&self) -> Option<Arc<dyn Connector>> {
        let cfg = self.config.read().await;
        self.entries
            .iter()
            .find(|e| e.connector.slicer().is_some() && cfg.cap_on(e.connector.descriptor(), Cap::SlicerOpen))
            .map(|e| e.connector.clone())
    }

    pub async fn list(&self) -> Vec<ConnectorInfo> {
        let cfg = self.config().await;
        let mut out = Vec::new();
        for e in &self.entries {
            let d = e.connector.descriptor();
            out.push(ConnectorInfo {
                id: d.id,
                name: d.name,
                kind: d.kind,
                auth: d.auth,
                enabled: cfg.is_enabled(d),
                caps: d
                    .caps
                    .iter()
                    .map(|c| CapInfo {
                        cap: *c,
                        write: c.is_write(),
                        on: cfg.cap_on(d, *c),
                        value: cfg.get(d.id).caps.get(c).copied().unwrap_or(c.default_on()),
                    })
                    .collect(),
                status: e.connector.status(&e.ctx).await,
            });
        }
        out
    }
}
