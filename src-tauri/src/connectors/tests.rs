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

/// alle Plattformen, damit die Tests auch in der CI (Linux) laufen
const BOTH: &[Platform] = &[Platform::MacOs, Platform::Windows, Platform::Linux];

static TEST: Descriptor = Descriptor {
    id: "test",
    name: "Test-Dienst",
    kind: Kind::Contacts,
    platforms: BOTH,
    auth: AuthKind::None,
    caps: &[Cap::ContactsRead, Cap::SpoolsRead, Cap::SpoolsWrite],
    base_url: "http://127.0.0.1:9",
    write_paths: &["/v1/items"],
    read_post_paths: &[],
    timeout_secs: 5,
    default_enabled: false,
    min_interval_ms: 0,
    intervals: &[(Cap::ContactsRead, 60), (Cap::SpoolsRead, 600), (Cap::SpoolsWrite, 1)],
    secrets: &["token"],
    heartbeat_secs: 0,
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
    read_post_paths: &[],
    timeout_secs: 5,
    default_enabled: false,
    min_interval_ms: 0,
    intervals: &[],
    secrets: &["token"],
    heartbeat_secs: 0,
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
    async fn sync(&self, _ctx: &ConnCtx, cap: Cap) -> Result<SyncOutcome> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(crate::error::AppError::Invalid("kaputt".into()));
        }
        Ok(SyncOutcome::changed(vec![format!("{cap:?} abgeglichen")]))
    }
}

fn registry(dir: &std::path::Path, backend: Arc<MemoryBackend>, list: Vec<Arc<dyn Connector>>) -> Registry {
    Registry::new(dir.to_path_buf(), backend, list).unwrap()
}

#[test]
fn werkszustand_lesen_an_schreiben_aus() {
    let mut cfg = ConnectorsConfig::default();
    assert!(!cfg.cap_on(&TEST, Cap::ContactsRead), "aus, solange die Anbindung aus ist");
    cfg.entry("test").enabled = Some(true);
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

    assert!(r.config().await.is_enabled(&OTHER), "Verbinden schaltet ein");
    r.disconnect("test").await.unwrap();
    assert_eq!(backend.get("test:token").unwrap(), None);
    assert_eq!(backend.get("other:token").unwrap().as_deref(), Some("geheim"));
    assert!(!r.config().await.is_enabled(&TEST), "Trennen schaltet aus");
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
    assert!(tick.changed);
    // nach 61 s nur die Kontakte (60 s), Spulen erst nach 600 s
    assert_eq!(s.run_once(&r, t0 + Duration::from_secs(61)).await.ran, vec![("test", Cap::ContactsRead)]);
    // Lesen ausgeschaltet → übersprungen
    r.set_cap("test", Cap::ContactsRead, false).await.unwrap();
    assert_eq!(s.run_once(&r, t0 + Duration::from_secs(601)).await.ran, vec![("test", Cap::SpoolsRead)]);
    assert_eq!(fake.syncs.load(Ordering::SeqCst), 4);
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

static HTTPD: Descriptor = Descriptor {
    id: "httpd",
    name: "Test-Dienst",
    kind: Kind::Catalog,
    platforms: BOTH,
    auth: AuthKind::None,
    caps: &[Cap::LinksRead, Cap::SpoolsWrite],
    base_url: "http://127.0.0.1:9",
    write_paths: &["/v1/items"],
    read_post_paths: &["/graphql/"],
    timeout_secs: 5,
    default_enabled: true,
    min_interval_ms: 120,
    intervals: &[],
    secrets: &[],
    heartbeat_secs: 0,
};

static WEBD: Descriptor = Descriptor {
    id: "webd",
    name: "Web",
    kind: Kind::Catalog,
    platforms: BOTH,
    auth: AuthKind::None,
    caps: &[Cap::LinksRead],
    base_url: "",
    write_paths: &[],
    read_post_paths: &[],
    timeout_secs: 5,
    default_enabled: true,
    min_interval_ms: 0,
    intervals: &[],
    secrets: &[],
    heartbeat_secs: 0,
};

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
    Mock::given(method("POST")).and(path("/graphql/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": {"ok": true}})))
        .mount(&server).await;

    let h = HttpGateway::with_base(&HTTPD, &server.uri());
    let t = Instant::now();
    assert_eq!(h.get_json("/v1/items?x=1", Some("abc")).await.unwrap()["n"], 1);
    assert_eq!(h.get_json("/v1/alt", None).await, Err(HttpError::Expired("Test-Dienst")));
    assert!(t.elapsed() >= Duration::from_millis(120), "Drosselung zwischen zwei Anfragen");

    // POST als Abfrage: ohne Schreibschalter erlaubt, aber nur auf read_post_paths
    assert_eq!(h.query_json("/graphql/", &serde_json::json!({"query": "{}"}), None).await.unwrap()["data"]["ok"], true);
    assert!(matches!(h.query_json("/v1/items", &body, None).await, Err(HttpError::WriteBlocked(..))));

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

#[tokio::test]
async fn fremde_hosts_nur_ohne_basis_und_ohne_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/bild.png")).respond_with(ResponseTemplate::new(200).set_body_bytes(vec![1u8, 2, 3])).mount(&server).await;
    Mock::given(method("GET")).and(path("/weg")).respond_with(ResponseTemplate::new(404)).mount(&server).await;
    let abs = format!("{}/bild.png", server.uri());

    // Anbindung mit eigener Basis: keine fremden Hosts (Token könnte sonst abfließen)
    let api = HttpGateway::with_base(&HTTPD, "https://api.example.com");
    assert_eq!(api.get_bytes(&abs, None).await, Err(HttpError::HostBlocked("Test-Dienst")));
    // Web-Zugang: fremde Hosts ja, aber nie mit Token; relative Pfade ergeben keinen Sinn
    let web = HttpGateway::new(&WEBD);
    assert_eq!(web.get_bytes(&abs, None).await.unwrap(), vec![1, 2, 3]);
    assert_eq!(web.get_bytes(&abs, Some("geheim")).await, Err(HttpError::HostBlocked("Web")));
    assert_eq!(web.get_bytes("/bild.png", None).await, Err(HttpError::HostBlocked("Web")));
    assert_eq!(web.get_bytes(&format!("{}/weg", server.uri()), None).await, Err(HttpError::Http("Web", 404)));
    let (status, _) = web.get_text(&format!("{}/weg", server.uri())).await.unwrap();
    assert_eq!(status, 404, "Seitenvorschau bekommt den Status statt eines Fehlers");
    // keine Anfrage mit Token ging an den Server
    assert!(server.received_requests().await.unwrap().iter().all(|r| !r.headers.contains_key("authorization")));
}

#[tokio::test]
async fn oeffentliche_anbindung_ist_ab_werk_an() {
    let dir = tempfile::tempdir().unwrap();
    let r = registry(dir.path(), Arc::new(MemoryBackend::default()), vec![Arc::new(PublicConnector(&WEBD))]);
    let info = r.list().await;
    assert!(info[0].enabled && info[0].status.connected);
    assert!(r.cap_on("webd", Cap::LinksRead).await);
    // Schalter setzen schaltet die Anbindung nicht aus
    r.set_cap("webd", Cap::LinksRead, true).await.unwrap();
    assert!(r.config().await.is_enabled(&WEBD));
    r.set_enabled("webd", false).await.unwrap();
    assert!(!r.cap_on("webd", Cap::LinksRead).await);
}

static BEAT: Descriptor = Descriptor {
    id: "beat",
    name: "Takt-Dienst",
    kind: Kind::PrinterCloud,
    platforms: &[Platform::MacOs, Platform::Windows, Platform::Linux],
    auth: AuthKind::None,
    caps: &[Cap::PrintersRead],
    base_url: "",
    write_paths: &[],
    read_post_paths: &[],
    timeout_secs: 5,
    default_enabled: false,
    min_interval_ms: 0,
    intervals: &[(Cap::PrintersRead, 60)],
    secrets: &[],
    heartbeat_secs: 1800,
};

/// Anbindung mit schnellerem Takt, solange `busy` gesetzt ist, und einer Anmeldeprüfung
struct Busy {
    busy: AtomicUsize,
    syncs: AtomicUsize,
    beats: AtomicUsize,
}

#[async_trait]
impl Connector for Busy {
    fn descriptor(&self) -> &'static Descriptor {
        &BEAT
    }
    async fn status(&self, _ctx: &ConnCtx) -> Status {
        Status { connected: true, ..Default::default() }
    }
    async fn connect(&self, _ctx: &ConnCtx) -> Result<()> {
        Ok(())
    }
    async fn interval(&self, _ctx: &ConnCtx, cap: Cap) -> Option<u64> {
        if self.busy.load(Ordering::SeqCst) > 0 { Some(20) } else { BEAT.interval(cap) }
    }
    async fn heartbeat(&self, _ctx: &ConnCtx) -> Result<SyncOutcome> {
        self.beats.fetch_add(1, Ordering::SeqCst);
        Ok(SyncOutcome { messages: vec!["Anmeldung abgelaufen".into()], changed: false })
    }
    async fn sync(&self, _ctx: &ConnCtx, _cap: Cap) -> Result<SyncOutcome> {
        self.syncs.fetch_add(1, Ordering::SeqCst);
        Ok(SyncOutcome::default())
    }
}

#[tokio::test]
async fn dynamischer_takt_und_anmeldepruefung() {
    let dir = tempfile::tempdir().unwrap();
    let c = Arc::new(Busy { busy: AtomicUsize::new(0), syncs: AtomicUsize::new(0), beats: AtomicUsize::new(0) });
    let r = registry(dir.path(), Arc::new(MemoryBackend::default()), vec![c.clone()]);
    let s = Scheduler::default();
    let t0 = Instant::now();
    // aus: weder Abgleich noch Anmeldeprüfung
    assert!(s.run_once(&r, t0).await.messages.is_empty());
    assert_eq!(c.beats.load(Ordering::SeqCst), 0);

    r.set_enabled("beat", true).await.unwrap();
    let tick = s.run_once(&r, t0).await;
    assert_eq!(tick.messages, vec!["Anmeldung abgelaufen".to_string()]);
    assert!(!tick.changed);
    // ruhig: nach 30 s noch nicht fällig (60 s)
    assert!(s.run_once(&r, t0 + Duration::from_secs(30)).await.ran.is_empty());
    // während eines Drucks: 20 s
    c.busy.store(1, Ordering::SeqCst);
    assert_eq!(s.run_once(&r, t0 + Duration::from_secs(30)).await.ran, vec![("beat", Cap::PrintersRead)]);
    assert_eq!(c.syncs.load(Ordering::SeqCst), 2);
    // Anmeldeprüfung erst nach 30 Minuten wieder
    assert_eq!(c.beats.load(Ordering::SeqCst), 1);
    s.run_once(&r, t0 + Duration::from_secs(1801)).await;
    assert_eq!(c.beats.load(Ordering::SeqCst), 2);
}
