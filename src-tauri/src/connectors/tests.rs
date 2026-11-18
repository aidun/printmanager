//! Contract-Tests der Anbindungs-Schicht mit einer Test-Anbindung (offline, Schlüsselbund durch Speicher ersetzt).

use super::config::ConnectorsConfig;
use super::http::{HttpError, HttpGateway};
use super::registry::Registry;
use super::scheduler::{due, Scheduler};
use super::secrets::{MemoryBackend, SecretBackend};
use super::*;
use reqwest::Method;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BOTH: &[Platform] = &[Platform::MacOs, Platform::Windows];

static TEST: Descriptor = Descriptor {
    id: "test",
    name: "Test-Dienst",
    kind: Kind::Contacts,
    platforms: BOTH,
    auth: AuthKind::AppPassword,
    caps: &[Cap::ContactsRead, Cap::SpoolsRead, Cap::SpoolsWrite],
    base_url: "http://127.0.0.1:9",
    write_paths: &["/v1/items"],
    min_interval_ms: 0,
    intervals: &[(Cap::ContactsRead, 60), (Cap::SpoolsRead, 600), (Cap::SpoolsWrite, 1)],
    secrets: &["token"],
};

static OTHER: Descriptor = Descriptor {
    id: "other",
    name: "Anderer Dienst",
    kind: Kind::Slicer,
    platforms: BOTH,
    auth: AuthKind::None,
    caps: &[Cap::SlicerOpen],
    base_url: "",
    write_paths: &[],
    min_interval_ms: 0,
    intervals: &[],
    secrets: &["token"],
};

/// Test-Anbindung: verbunden, sobald ein Token im Schlüsselbund liegt; zählt Abgleiche
struct Fake {
    d: &'static Descriptor,
    syncs: AtomicUsize,
    fail: bool,
}

impl Fake {
    fn new(d: &'static Descriptor) -> Arc<Self> {
        Arc::new(Self { d, syncs: AtomicUsize::new(0), fail: false })
    }
}

#[async_trait]
impl Connector for Fake {
    fn descriptor(&self) -> &'static Descriptor {
        self.d
    }
    async fn status(&self, ctx: &ConnCtx) -> Status {
        Status { connected: ctx.secrets.get("token").ok().flatten().is_some(), ..Default::default() }
    }
    async fn connect(&self, ctx: &ConnCtx) -> Result<()> {
        ctx.secrets.set("token", "geheim")
    }
    async fn sync(&self, _ctx: &ConnCtx, cap: Cap) -> Result<Vec<String>> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(crate::error::AppError::Invalid("kaputt".into()));
        }
        Ok(vec![format!("{cap:?} abgeglichen")])
    }
}

fn registry(dir: &std::path::Path, backend: Arc<MemoryBackend>, list: Vec<Arc<dyn Connector>>) -> Registry {
    Registry::new(dir.to_path_buf(), backend, list).unwrap()
}

#[test]
fn werkszustand_lesen_an_schreiben_aus() {
    let mut cfg = ConnectorsConfig::default();
    assert!(!cfg.cap_on(&TEST, Cap::ContactsRead), "aus, solange die Anbindung aus ist");
    cfg.entry("test").enabled = true;
    assert!(cfg.cap_on(&TEST, Cap::ContactsRead));
    assert!(!cfg.cap_on(&TEST, Cap::SpoolsWrite), "Schreiben ab Werk aus");
    assert!(!cfg.cap_on(&TEST, Cap::PrintersRead), "Fähigkeit, die die Anbindung nicht hat");
    cfg.entry("test").caps.insert(Cap::SpoolsWrite, true);
    cfg.entry("test").caps.insert(Cap::ContactsRead, false);
    assert!(cfg.cap_on(&TEST, Cap::SpoolsWrite));
    assert!(!cfg.cap_on(&TEST, Cap::ContactsRead));
}

#[tokio::test]
async fn einstellungen_bleiben_erhalten_und_schalten_schreiben() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemoryBackend::default());
    let r = registry(dir.path(), backend.clone(), vec![Fake::new(&TEST)]);
    let http = r.get("test").unwrap().ctx.http.clone().unwrap();
    assert!(!http.write_enabled());

    r.set_enabled("test", true).await.unwrap();
    assert!(!http.write_enabled(), "an, aber Schreiben bleibt aus");
    r.set_cap("test", Cap::SpoolsWrite, true).await.unwrap();
    assert!(http.write_enabled());
    assert!(r.set_cap("test", Cap::PrintersRead, true).await.is_err(), "fremde Fähigkeit");
    assert!(r.set_enabled("gibtsnicht", true).await.is_err());

    // neu laden: Schalter aus connectors.json
    let r2 = registry(dir.path(), backend, vec![Fake::new(&TEST)]);
    assert!(r2.cap_on("test", Cap::SpoolsWrite).await);
    assert!(r2.get("test").unwrap().ctx.http.as_ref().unwrap().write_enabled());
    let info = r2.list().await;
    assert_eq!(info.len(), 1);
    assert!(info[0].enabled && !info[0].status.connected);
    assert!(info[0].caps.iter().any(|c| c.cap == Cap::SpoolsWrite && c.write && c.on));
}

#[tokio::test]
async fn doppelte_anbindung_wird_abgelehnt() {
    let dir = tempfile::tempdir().unwrap();
    let r = Registry::new(dir.path().to_path_buf(), Arc::new(MemoryBackend::default()), vec![Fake::new(&TEST), Fake::new(&TEST)]);
    assert!(r.is_err());
}

#[tokio::test]
async fn trennen_loescht_nur_eigene_geheimnisse() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemoryBackend::default());
    let r = registry(dir.path(), backend.clone(), vec![Fake::new(&TEST), Fake::new(&OTHER)]);
    r.set_enabled("test", true).await.unwrap();
    r.connect("test").await.unwrap();
    r.connect("other").await.unwrap();
    assert_eq!(backend.get("test:token").unwrap().as_deref(), Some("geheim"), "Konto <anbindung>:<name>");
    assert!(r.list().await.iter().find(|i| i.id == "test").unwrap().status.connected);

    r.disconnect("test").await.unwrap();
    assert_eq!(backend.get("test:token").unwrap(), None);
    assert_eq!(backend.get("other:token").unwrap().as_deref(), Some("geheim"));
    assert!(!r.config().await.enabled("test"), "Trennen schaltet aus");
    // nur deklarierte Geheimnisse
    assert!(r.get("test").unwrap().ctx.secrets.set("fremd", "x").is_err());
}

#[test]
fn faelligkeit() {
    let t0 = Instant::now();
    assert!(due(None, t0, Duration::from_secs(60)));
    assert!(!due(Some(t0), t0 + Duration::from_secs(59), Duration::from_secs(60)));
    assert!(due(Some(t0), t0 + Duration::from_secs(60), Duration::from_secs(60)));
}

#[tokio::test]
async fn takt_nur_fuer_eingeschaltete_verbundene_und_faellige_lesende_faehigkeiten() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::new(&TEST);
    let r = registry(dir.path(), Arc::new(MemoryBackend::default()), vec![fake.clone()]);
    let s = Scheduler::default();
    let t0 = Instant::now();

    // aus → nichts
    assert!(s.run_once(&r, t0).await.ran.is_empty());
    // an, aber nicht verbunden → nichts
    r.set_enabled("test", true).await.unwrap();
    r.set_cap("test", Cap::SpoolsWrite, true).await.unwrap();
    assert!(s.run_once(&r, t0).await.ran.is_empty());
    // verbunden → beide lesenden Fähigkeiten, nie die schreibende
    r.connect("test").await.unwrap();
    let tick = s.run_once(&r, t0).await;
    assert_eq!(tick.ran, vec![("test", Cap::ContactsRead), ("test", Cap::SpoolsRead)]);
    assert_eq!(tick.messages.len(), 2);
    // nach 61 s nur die Kontakte (60 s), Spulen erst nach 600 s
    assert_eq!(s.run_once(&r, t0 + Duration::from_secs(61)).await.ran, vec![("test", Cap::ContactsRead)]);
    // Lesen ausgeschaltet → übersprungen
    r.set_cap("test", Cap::ContactsRead, false).await.unwrap();
    assert_eq!(s.run_once(&r, t0 + Duration::from_secs(601)).await.ran, vec![("test", Cap::SpoolsRead)]);
    // reset → sofort wieder fällig
    s.reset("test").await;
    assert_eq!(s.run_once(&r, t0 + Duration::from_secs(602)).await.ran, vec![("test", Cap::SpoolsRead)]);
    assert_eq!(fake.syncs.load(Ordering::SeqCst), 5);
}

#[tokio::test]
async fn fehler_im_takt_werden_gemeldet_und_nicht_sofort_wiederholt() {
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(Fake { d: &TEST, syncs: AtomicUsize::new(0), fail: true });
    let r = registry(dir.path(), Arc::new(MemoryBackend::default()), vec![fake.clone()]);
    r.set_enabled("test", true).await.unwrap();
    r.connect("test").await.unwrap();
    let s = Scheduler::default();
    let t0 = Instant::now();
    let tick = s.run_once(&r, t0).await;
    assert_eq!(tick.errors.len(), 2);
    assert!(tick.errors[0].1.contains("kaputt"));
    assert!(s.run_once(&r, t0 + Duration::from_secs(1)).await.ran.is_empty());
}

#[tokio::test]
async fn http_lesen_drosseln_und_schreibschutz() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/v1/items")).and(header("authorization", "Bearer abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"n": 1})))
        .mount(&server).await;
    Mock::given(method("GET")).and(path("/v1/alt")).respond_with(ResponseTemplate::new(401)).mount(&server).await;
    let body = serde_json::json!({"id": 1});
    Mock::given(method("PUT")).and(path("/v1/items")).and(body_json(&body))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server).await;

    static WRITE: &[&str] = &["/v1/items"];
    let h = HttpGateway::new("Test-Dienst", &server.uri(), Duration::from_millis(120), WRITE);
    let t = Instant::now();
    assert_eq!(h.get_json("/v1/items?x=1", Some("abc")).await.unwrap()["n"], 1);
    assert_eq!(h.get_json("/v1/alt", None).await, Err(HttpError::Expired("Test-Dienst")));
    assert!(t.elapsed() >= Duration::from_millis(120), "Drosselung zwischen zwei Anfragen");

    // Schreiben: ohne Schalter blockiert, ohne dass eine Anfrage rausgeht
    assert!(matches!(h.send(Method::PUT, "/v1/items", Some(&body), None).await, Err(HttpError::WriteBlocked(..))));
    h.set_write_enabled(true);
    assert_eq!(h.send(Method::PUT, "/v1/items", Some(&body), None).await.unwrap(), (200, "{}".into()));
    // nicht freigegebener Pfad bleibt blockiert, auch mit Schalter
    assert!(matches!(h.send(Method::DELETE, "/v1/anderes", None, None).await, Err(HttpError::WriteBlocked(..))));
    assert!(matches!(h.send(Method::POST, "/v1/items/batch", None, None).await, Err(HttpError::WriteBlocked(..))));
    let puts = server.received_requests().await.unwrap().iter().filter(|r| r.method.as_str() == "PUT").count();
    assert_eq!(puts, 1, "der blockierte Versuch ging nicht raus");
}
