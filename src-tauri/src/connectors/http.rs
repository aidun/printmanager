//! HTTP-Zugang je Anbindung: gedrosselt, mit Zeitlimit und Schreibschutz.
//! - GET immer; POST als reine Abfrage nur auf `read_post_paths` (z.B. GraphQL).
//! - Alles andere nur mit eingeschaltetem Schreibschalter und auf `write_paths`
//!   (verallgemeinert den früheren Schutz `write_allowed` aus `bambu/`).
//! - Absolute Adressen (andere Hosts) nur bei Anbindungen ohne `base_url` und nie mit Token,
//!   damit ein Token nie an einen fremden Host geht.
//!
//! Fehlertexte enthalten weder Token noch URL-Parameter.

use super::Descriptor;
use reqwest::Method;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum HttpError {
    #[error("{0}: Anmeldung abgelaufen – bitte in den Einstellungen neu verbinden.")]
    Expired(&'static str),
    #[error("{0} antwortet mit Fehler {1}.")]
    Http(&'static str, u16),
    #[error("{0} nicht erreichbar: {1}")]
    Net(&'static str, String),
    #[error("Unerwartete Antwort von {0}.")]
    Parse(&'static str),
    #[error("{0}: Schreiben ist hier nicht erlaubt ({1}).")]
    WriteBlocked(&'static str, String),
    #[error("{0}: Diese Adresse ist hier nicht erlaubt.")]
    HostBlocked(&'static str),
}

impl From<HttpError> for crate::error::AppError {
    fn from(e: HttpError) -> Self {
        crate::error::AppError::Invalid(e.to_string())
    }
}

pub struct HttpGateway {
    /// Anzeigename für Fehlermeldungen
    name: &'static str,
    client: reqwest::Client,
    base: String,
    gap: Duration,
    last: Mutex<Option<Instant>>,
    write_paths: &'static [&'static str],
    read_post_paths: &'static [&'static str],
    write_enabled: AtomicBool,
}

fn is_absolute(target: &str) -> bool {
    target.starts_with("https://") || target.starts_with("http://")
}

fn path_only(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

impl HttpGateway {
    pub fn new(d: &'static Descriptor) -> Self {
        Self::with_base(d, d.base_url)
    }

    /// Wie `new`, aber mit anderer Basisadresse (Tests gegen einen lokalen Server)
    pub fn with_base(d: &'static Descriptor, base: &str) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Printmanager/", env!("CARGO_PKG_VERSION"), " (Desktop-App)"))
            .timeout(Duration::from_secs(d.timeout_secs.max(1)))
            .build()
            .unwrap_or_default();
        Self {
            name: d.name,
            client,
            base: base.trim_end_matches('/').to_string(),
            gap: Duration::from_millis(d.min_interval_ms),
            last: Mutex::new(None),
            write_paths: d.write_paths,
            read_post_paths: d.read_post_paths,
            write_enabled: AtomicBool::new(false),
        }
    }

    /// Schalter „Schreiben“ der Anbindung (von der Registry gesetzt)
    pub fn set_write_enabled(&self, on: bool) {
        self.write_enabled.store(on, Ordering::SeqCst);
    }

    pub fn write_enabled(&self) -> bool {
        self.write_enabled.load(Ordering::SeqCst)
    }

    /// Darf diese Anfrage raus? GET immer; POST-Abfragen auf `read_post_paths`;
    /// alles andere nur mit Schalter und auf `write_paths`.
    pub fn allowed(&self, method: &Method, path: &str) -> bool {
        let p = path_only(path);
        *method == Method::GET
            || (*method == Method::POST && self.read_post_paths.contains(&p))
            || (self.write_enabled() && self.write_paths.contains(&p))
    }

    /// Ziel-URL bilden. Absolute Adressen nur ohne `base_url` und ohne Token.
    fn url(&self, target: &str, token: Option<&str>) -> Result<String, HttpError> {
        if is_absolute(target) {
            if !self.base.is_empty() || token.is_some() {
                return Err(HttpError::HostBlocked(self.name));
            }
            return Ok(target.to_string());
        }
        if self.base.is_empty() {
            return Err(HttpError::HostBlocked(self.name));
        }
        Ok(format!("{}{target}", self.base))
    }

    async fn throttle(&self) {
        let mut last = self.last.lock().await;
        if let Some(t) = *last {
            let wait = self.gap.saturating_sub(t.elapsed());
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
        }
        *last = Some(Instant::now());
    }

    fn net(&self, e: reqwest::Error) -> HttpError {
        HttpError::Net(self.name, e.without_url().to_string())
    }

    async fn request(&self, method: Method, target: &str, body: Option<&Value>, token: Option<&str>) -> Result<reqwest::Response, HttpError> {
        if !self.allowed(&method, target) {
            return Err(HttpError::WriteBlocked(self.name, format!("{method} {}", path_only(target))));
        }
        let url = self.url(target, token)?;
        self.throttle().await;
        let mut req = self.client.request(method, url);
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        req.send().await.map_err(|e| self.net(e))
    }

    /// GET und JSON lesen. 401/403 bedeuten: Anmeldung abgelaufen.
    pub async fn get_json(&self, target: &str, token: Option<&str>) -> Result<Value, HttpError> {
        let r = self.request(Method::GET, target, None, token).await?;
        self.json(r).await
    }

    /// POST-Abfrage (z.B. GraphQL) und JSON lesen; nur auf `read_post_paths`.
    pub async fn query_json(&self, target: &str, body: &Value, token: Option<&str>) -> Result<Value, HttpError> {
        let r = self.request(Method::POST, target, Some(body), token).await?;
        self.json(r).await
    }

    async fn json(&self, r: reqwest::Response) -> Result<Value, HttpError> {
        match r.status().as_u16() {
            200..=299 => r.json::<Value>().await.map_err(|_| HttpError::Parse(self.name)),
            401 | 403 => Err(HttpError::Expired(self.name)),
            s => Err(HttpError::Http(self.name, s)),
        }
    }

    /// GET und Text lesen; liefert auch Fehlerstatus, ohne abzubrechen (z.B. für Seitenvorschauen).
    pub async fn get_text(&self, target: &str) -> Result<(u16, String), HttpError> {
        let r = self.request(Method::GET, target, None, None).await?;
        let status = r.status().as_u16();
        Ok((status, r.text().await.unwrap_or_default()))
    }

    /// GET und Bytes lesen (Downloads, Bilder); Fehlerstatus ist ein Fehler.
    pub async fn get_bytes(&self, target: &str, token: Option<&str>) -> Result<Vec<u8>, HttpError> {
        let r = self.request(Method::GET, target, None, token).await?;
        match r.status().as_u16() {
            200..=299 => Ok(r.bytes().await.map_err(|e| self.net(e))?.to_vec()),
            401 | 403 => Err(HttpError::Expired(self.name)),
            s => Err(HttpError::Http(self.name, s)),
        }
    }

    /// Beliebige Methode mit optionalem JSON-Inhalt; liefert Status und Antworttext.
    /// Schreibende Anfragen prüft `allowed` vor dem Senden.
    pub async fn send(&self, method: Method, path: &str, body: Option<&Value>, token: Option<&str>) -> Result<(u16, String), HttpError> {
        let r = self.request(method, path, body, token).await?;
        let status = r.status().as_u16();
        if status == 401 || status == 403 {
            return Err(HttpError::Expired(self.name));
        }
        Ok((status, r.text().await.unwrap_or_default()))
    }
}
