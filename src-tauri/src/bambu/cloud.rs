//! HTTP-Zugang zur Bambu-Cloud (`api.bambulab.com`). Nur lesende GET-Anfragen, gedrosselt.
//! Die Endpunkte sind inoffiziell (siehe docs/history/PLAN-Bambu-Anbindung.md) und werden tolerant gelesen.

use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub const GLOBAL: &str = "https://api.bambulab.com";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum CloudError {
    #[error("Bambu-Anmeldung abgelaufen – bitte in den Einstellungen neu anmelden.")]
    Expired,
    #[error("Bambu-Konto antwortet mit Fehler {0}.")]
    Http(u16),
    #[error("Bambu-Konto nicht erreichbar: {0}")]
    Net(String),
    #[error("Unerwartete Antwort vom Bambu-Konto.")]
    Parse,
}

impl From<CloudError> for crate::error::AppError {
    fn from(e: CloudError) -> Self {
        crate::error::AppError::Invalid(e.to_string())
    }
}

pub struct Cloud {
    http: reqwest::Client,
    base: String,
    /// Zeitpunkt der letzten Anfrage, damit höchstens eine Anfrage pro `gap` rausgeht
    last: Mutex<Option<Instant>>,
    gap: Duration,
}

impl Cloud {
    pub fn new(base: &str, gap: Duration) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Printmanager/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("HTTP-Client");
        Self { http, base: base.trim_end_matches('/').to_string(), last: Mutex::new(None), gap }
    }

    /// GET `path` mit dem Token. 401/403 bedeuten: Anmeldung abgelaufen.
    pub async fn get(&self, token: &str, path: &str) -> Result<Value, CloudError> {
        {
            let mut last = self.last.lock().await;
            if let Some(t) = *last {
                let wait = self.gap.saturating_sub(t.elapsed());
                if !wait.is_zero() {
                    tokio::time::sleep(wait).await;
                }
            }
            *last = Some(Instant::now());
        }
        let r = self
            .http
            .get(format!("{}{}", self.base, path))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| CloudError::Net(e.without_url().to_string()))?;
        match r.status().as_u16() {
            200..=299 => r.json::<Value>().await.map_err(|_| CloudError::Parse),
            401 | 403 => Err(CloudError::Expired),
            s => Err(CloudError::Http(s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn anfragen_fehler_und_drosselung() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/ok"))
            .and(header("authorization", "Bearer abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"uid": 7})))
            .mount(&server)
            .await;
        Mock::given(method("GET")).and(path("/v1/alt")).respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(method("GET")).and(path("/v1/kaputt")).respond_with(ResponseTemplate::new(500)).mount(&server).await;

        let c = Cloud::new(&server.uri(), Duration::from_millis(150));
        let t = Instant::now();
        assert_eq!(c.get("abc", "/v1/ok").await.unwrap()["uid"], 7);
        assert_eq!(c.get("abc", "/v1/alt").await, Err(CloudError::Expired));
        assert_eq!(c.get("abc", "/v1/kaputt").await, Err(CloudError::Http(500)));
        assert!(t.elapsed() >= Duration::from_millis(300), "zwei Pausen zwischen drei Anfragen");
    }
}
