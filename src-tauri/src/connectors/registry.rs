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
    pub on: bool,
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
            let http = d.caps.iter().any(|c| *c != Cap::SlicerOpen)
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

    /// Mehrere Schalter auf einmal setzen (eine Schreiboperation)
    pub async fn set_caps(&self, id: &str, caps: &[(Cap, bool)]) -> Result<()> {
        let d = self.get(id)?.connector.descriptor();
        if let Some((c, _)) = caps.iter().find(|(c, _)| !d.has(*c)) {
            return Err(AppError::Invalid(format!("Anbindung „{id}“ hat die Fähigkeit {c:?} nicht")));
        }
        self.update(|cfg| {
            let e = cfg.entry(id);
            for (c, on) in caps {
                e.caps.insert(*c, *on);
            }
        })
        .await
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

    pub async fn connect(&self, id: &str) -> Result<()> {
        let e = self.get(id)?;
        e.connector.connect(&e.ctx).await
    }

    /// Trennen: Geheimnisse löschen und die Anbindung ausschalten. Lokale Daten und Verknüpfungen bleiben.
    pub async fn disconnect(&self, id: &str) -> Result<()> {
        let e = self.get(id)?;
        e.connector.disconnect(&e.ctx).await?;
        self.set_enabled(id, false).await
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
                caps: d.caps.iter().map(|c| CapInfo { cap: *c, write: c.is_write(), on: cfg.cap_on(d, *c) }).collect(),
                status: e.connector.status(&e.ctx).await,
            });
        }
        out
    }
}
