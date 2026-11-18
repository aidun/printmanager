//! HTTP-Zugang je Anbindung: gedrosselt, mit Zeitlimit und Schreibschutz.
//! Nicht-GET-Anfragen gehen nur raus, wenn der Schreibschalter der Anbindung an ist und der Pfad
//! in `Descriptor::write_paths` steht (verallgemeinert den früheren Schutz `write_allowed` aus `bambu/`).
//! Fehlertexte enthalten weder Token noch URL-Parameter.

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
    write_enabled: AtomicBool,
}

impl HttpGateway {
    pub fn new(name: &'static str, base: &str, gap: Duration, write_paths: &'static [&'static str]) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Printmanager/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();
        Self { name, client, base: base.trim_end_matches('/').to_string(), gap, last: Mutex::new(None), write_paths, write_enabled: AtomicBool::new(false) }
    }

    /// Schalter „Schreiben“ der Anbindung (von der Registry gesetzt)
    pub fn set_write_enabled(&self, on: bool) {
        self.write_enabled.store(on, Ordering::SeqCst);
    }

    pub fn write_enabled(&self) -> bool {
        self.write_enabled.load(Ordering::SeqCst)
    }

    /// Darf diese Anfrage raus? GET immer; alles andere nur mit Schalter und auf erlaubtem Pfad.
    pub fn allowed(&self, method: &Method, path: &str) -> bool {
        *method == Method::GET || (self.write_enabled() && self.write_paths.contains(&path.split('?').next().unwrap_or(path)))
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

    /// GET und JSON lesen. 401/403 bedeuten: Anmeldung abgelaufen.
    pub async fn get_json(&self, path: &str, token: Option<&str>) -> Result<Value, HttpError> {
        self.throttle().await;
        let mut req = self.client.get(format!("{}{path}", self.base));
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        let r = req.send().await.map_err(|e| self.net(e))?;
        match r.status().as_u16() {
            200..=299 => r.json::<Value>().await.map_err(|_| HttpError::Parse(self.name)),
            401 | 403 => Err(HttpError::Expired(self.name)),
            s => Err(HttpError::Http(self.name, s)),
        }
    }

    /// Beliebige Methode mit optionalem JSON-Inhalt; liefert Status und Antworttext.
    /// Schreibende Anfragen prüft `allowed` vor dem Senden.
    pub async fn send(&self, method: Method, path: &str, body: Option<&Value>, token: Option<&str>) -> Result<(u16, String), HttpError> {
        if !self.allowed(&method, path) {
            return Err(HttpError::WriteBlocked(self.name, format!("{method} {}", path.split('?').next().unwrap_or(path))));
        }
        self.throttle().await;
        let mut req = self.client.request(method, format!("{}{path}", self.base));
        if let Some(t) = token {
            req = req.bearer_auth(t);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let r = req.send().await.map_err(|e| self.net(e))?;
        let status = r.status().as_u16();
        if status == 401 || status == 403 {
            return Err(HttpError::Expired(self.name));
        }
        Ok((status, r.text().await.unwrap_or_default()))
    }
}
