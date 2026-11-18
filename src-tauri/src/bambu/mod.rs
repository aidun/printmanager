//! Bambu-Konto als Anbindung `bambu-cloud` (docs/PLAN-Anbindungen.md, Stufe A3).
//! Anmeldung im Anmeldefenster von Bambu, Token im Schlüsselbund (`bambu-cloud:token`), HTTP über den
//! gemeinsamen, gedrosselten Zugang, Abgleiche über den gemeinsamen Takt (`connectors::start`).
//! Das Passwort sieht die App nie; das Token steht nie in Datenbank, Einstellungen oder Logs.
//! Schreiben nur auf die Spulenliste und nur mit eingeschaltetem Schalter (AGENTS.md Regel 2).

pub mod filament_write;
pub mod history;
pub mod makerworld;
pub mod printers;

use crate::commands::AppState;
use crate::connectors::config::ConnectorsConfig;
use crate::connectors::http::HttpError;
use crate::connectors::registry::Registry;
use crate::connectors::secrets::{SecretBackend, SecretScope};
use crate::connectors::{AuthKind, Cap, ConnCtx, Connector, Descriptor, Kind, Platform, Status, SyncOutcome};
use crate::error::{AppError, Result};
use crate::model::SyncResult;
use crate::watch::{Changed, EVENT};
use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::RwLock;

pub const ID: &str = "bambu-cloud";
const LOGIN_LABEL: &str = "bambu-login";
const LOGIN_URL: &str = "https://bambulab.com/en/sign-in";
/// früherer Schlüsselbund-Eintrag (bis 0.3), wird einmalig übernommen
const LEGACY_TOKEN_ACCOUNT: &str = "bambu-token";
/// frühere Einstellungsdatei (bis 0.3), wird einmalig in `connectors.json` übernommen
const LEGACY_CONFIG: &str = "bambu.json";
/// Gültigkeit, falls das Token kein Ablaufdatum verrät (laut Community ca. 90 Tage)
const DEFAULT_DAYS: i64 = 90;
/// „Jetzt abgleichen“ beim Öffnen der Filament-Seite höchstens so oft
const FILAMENT_FRESH_SECS: i64 = 120;
const FILAMENT_PATH: &str = "/v1/design-user-service/my/filament/v2";
const FILAMENT_BATCH_PATH: &str = "/v1/design-user-service/my/filament/v2/batch";

pub static DESCRIPTOR: Descriptor = Descriptor {
    id: ID,
    name: "Bambu-Konto",
    kind: Kind::PrinterCloud,
    platforms: &[Platform::MacOs, Platform::Windows, Platform::Linux],
    auth: AuthKind::WebviewCookie,
    caps: &[Cap::SpoolsRead, Cap::SpoolsWrite, Cap::PrintersRead, Cap::PrintHistoryRead, Cap::CatalogRead],
    base_url: "https://api.bambulab.com",
    // einzige Pfade, auf die geschrieben werden darf: die Spulenliste und ihr Löschaufruf
    write_paths: &[FILAMENT_PATH, FILAMENT_BATCH_PATH],
    min_interval_ms: 1000,
    intervals: &[
        (Cap::SpoolsRead, 15 * 60),
        (Cap::PrintersRead, 60), // während eines Drucks 20 s, siehe `interval`
        (Cap::PrintHistoryRead, 15 * 60),
        (Cap::CatalogRead, 6 * 3600),
    ],
    secrets: &["token"],
    heartbeat_secs: 30 * 60,
};

/// Nicht geheime Kontoangaben, gespeichert als Optionen der Anbindung in `connectors.json`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Account {
    region: String,
    account_name: Option<String>,
    uid: Option<i64>,
    expires_at: Option<String>,
}

/// Sicht der Oberfläche (Einstellungen → Bambu-Konto): Kontoangaben und Schalter
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BambuConfig {
    pub region: String,
    pub account_name: Option<String>,
    pub uid: Option<i64>,
    pub expires_at: Option<String>,
    /// Filament aus dem Konto abgleichen
    pub filament: bool,
    /// Drucker und Druckstatus abfragen (mit Druckverlauf)
    pub printers: bool,
    /// MakerWorld-Sammlungen in den Katalog übernehmen
    pub makerworld: bool,
    /// Änderungen an Spulen ins Bambu-Konto schreiben (ab Werk aus)
    pub filament_write: bool,
}

/// Laufzeitzustand (nur im Speicher)
#[derive(Debug, Clone, Default)]
struct Runtime {
    /// Token vorhanden und nicht als abgelaufen bekannt
    connected: bool,
    expired: bool,
    last_ok_at: Option<String>,
    last_error: Option<String>,
    /// Anmeldefenster ist gerade offen
    logging_in: bool,
    /// letzter erfolgreicher Filament-Abgleich mit dem Konto
    filament_synced_at: Option<String>,
    /// letzter MakerWorld-Abgleich
    mw_synced_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BambuStatus {
    #[serde(flatten)]
    pub config: BambuConfig,
    pub connected: bool,
    pub expired: bool,
    pub last_ok_at: Option<String>,
    pub last_error: Option<String>,
    pub logging_in: bool,
    pub filament_synced_at: Option<String>,
    pub mw_synced_at: Option<String>,
}

/// Innenleben der Anbindung; die Bambu-Module (Drucker, Verlauf, MakerWorld, Schreiben) greifen darauf zu.
pub struct Bambu {
    ctx: ConnCtx,
    registry: Arc<Registry>,
    token: RwLock<Option<String>>,
    status: RwLock<Runtime>,
    /// zuletzt gesehene Drucker (nur im Speicher)
    pub printers: RwLock<Vec<printers::PrinterView>>,
    pub printers_at: RwLock<Option<String>>,
    /// Druckverlauf einmal vollständig abgleichen, danach nur die neuesten Einträge
    history_full_done: AtomicBool,
}

impl Bambu {
    pub fn new(registry: Arc<Registry>) -> Result<Self> {
        let ctx = registry.get(ID)?.ctx.clone();
        let token = ctx.secrets.get("token").ok().flatten().filter(|t| !t.is_empty());
        let status = Runtime { connected: token.is_some(), ..Default::default() };
        Ok(Self {
            ctx,
            registry,
            token: RwLock::new(token),
            status: RwLock::new(status),
            printers: RwLock::new(Vec::new()),
            printers_at: RwLock::new(None),
            history_full_done: AtomicBool::new(false),
        })
    }

    async fn account(&self) -> Account {
        let opts = self.registry.config().await.get(ID).options;
        serde_json::from_value(opts).unwrap_or_default()
    }

    async fn set_account(&self, a: &Account) -> Result<()> {
        self.registry.set_options(ID, serde_json::to_value(a).unwrap_or_default()).await
    }

    pub async fn status(&self) -> BambuStatus {
        let cfg = self.registry.config().await;
        let a = self.account().await;
        let rt = self.status.read().await.clone();
        BambuStatus {
            config: BambuConfig {
                region: if a.region.is_empty() { "global".into() } else { a.region },
                account_name: a.account_name,
                uid: a.uid,
                expires_at: a.expires_at,
                filament: cfg.cap_on(&DESCRIPTOR, Cap::SpoolsRead),
                printers: cfg.cap_on(&DESCRIPTOR, Cap::PrintersRead),
                makerworld: cfg.cap_on(&DESCRIPTOR, Cap::CatalogRead),
                filament_write: cfg.cap_on(&DESCRIPTOR, Cap::SpoolsWrite),
            },
            connected: rt.connected,
            expired: rt.expired,
            last_ok_at: rt.last_ok_at,
            last_error: rt.last_error,
            logging_in: rt.logging_in,
            filament_synced_at: rt.filament_synced_at,
            mw_synced_at: rt.mw_synced_at,
        }
    }

    async fn token(&self) -> Result<String> {
        self.token.read().await.clone().ok_or_else(|| AppError::Invalid("Nicht am Bambu-Konto angemeldet.".into()))
    }

    fn http(&self) -> Result<&crate::connectors::http::HttpGateway> {
        self.ctx.http.as_deref().ok_or_else(|| AppError::Invalid("Bambu-Konto: kein HTTP-Zugang".into()))
    }

    /// Erfolg, Fehler und Ablauf einer Anfrage im Zustand vermerken.
    async fn note<T>(&self, r: &std::result::Result<T, HttpError>) {
        let mut st = self.status.write().await;
        match r {
            Ok(_) => {
                st.last_ok_at = Some(crate::store::now());
                st.last_error = None;
                st.expired = false;
            }
            Err(e @ HttpError::Expired(_)) => {
                st.expired = true;
                st.connected = false;
                st.last_error = Some(e.to_string());
            }
            Err(HttpError::WriteBlocked(..)) => {}
            Err(e) => st.last_error = Some(e.to_string()),
        }
    }

    /// GET mit dem gespeicherten Token.
    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let token = self.token().await?;
        let r = self.http()?.get_json(path, Some(&token)).await;
        self.note(&r).await;
        Ok(r?)
    }

    /// Schreibende Anfrage. Der HTTP-Zugang lässt sie nur auf die Spulenliste und nur mit Schalter durch.
    pub async fn send_raw(&self, method: reqwest::Method, path: &str, body: Option<&serde_json::Value>) -> Result<(u16, String)> {
        let token = self.token().await?;
        let r = self.http()?.send(method, path, body, Some(&token)).await;
        self.note(&r).await;
        Ok(r?)
    }

    /// Token übernehmen: im Schlüsselbund speichern und Kontoangaben abrufen.
    async fn accept_token(&self, token: String) -> Result<String> {
        self.ctx.secrets.set("token", &token)?;
        *self.token.write().await = Some(token.clone());
        {
            let mut st = self.status.write().await;
            st.connected = true;
            st.expired = false;
            st.last_error = None;
        }
        self.registry.set_enabled(ID, true).await?;
        let expires = token_expiry(&token);
        let profile = self.get("/v1/design-user-service/my/profile").await.ok();
        let name = profile
            .as_ref()
            .and_then(|p| p.get("name").or_else(|| p.get("handle")))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let uid = profile.as_ref().and_then(|p| p.get("uid").or_else(|| p.get("identifier"))).and_then(|v| v.as_i64());
        let uid = match uid {
            Some(u) => Some(u),
            None => self.get("/v1/design-user-service/my/preference").await.ok().and_then(|p| p.get("uid")?.as_i64()),
        };
        let a = Account { region: "global".into(), account_name: name.clone(), uid, expires_at: Some(expires) };
        self.set_account(&a).await?;
        Ok(name.unwrap_or_else(|| "Bambu-Konto".into()))
    }

    /// Abmelden: Token löschen, Kontoangaben vergessen. Die Schalter bleiben für die nächste Anmeldung.
    async fn logout(&self) -> Result<()> {
        self.ctx.secrets.clear()?;
        crate::connectors::secrets::KeyringBackend.delete(LEGACY_TOKEN_ACCOUNT)?;
        *self.token.write().await = None;
        {
            let mut st = self.status.write().await;
            st.connected = false;
            st.expired = false;
            st.last_error = None;
            st.last_ok_at = None;
        }
        self.set_account(&Account { region: "global".into(), ..Default::default() }).await
    }

    async fn active(&self, cap: Cap) -> bool {
        self.status.read().await.connected && self.registry.cap_on(ID, cap).await
    }

    /// Filament kommt aus dem Konto (angemeldet und Schalter an). Dann ruht der Abgleich mit spools.json.
    pub async fn filament_active(&self) -> bool {
        self.active(Cap::SpoolsRead).await
    }

    async fn printing(&self) -> bool {
        self.printers.read().await.iter().any(|p| printers::active(&p.state))
    }

    /// Anmeldeprüfung: meldet eine abgelaufene Anmeldung.
    async fn check_login(&self) -> Vec<String> {
        if !self.status.read().await.connected {
            return vec![];
        }
        let _ = self.get("/v1/design-user-service/my/preference").await;
        if self.status.read().await.expired {
            vec![HttpError::Expired(DESCRIPTOR.name).to_string()]
        } else {
            vec![]
        }
    }
}

// ---------- Anbindung ----------

/// Bambu-Konto als `Connector` für Registry und Takt
pub struct BambuConnector {
    app: AppHandle,
}

impl BambuConnector {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn bambu(&self) -> State<'_, Bambu> {
        self.app.state::<Bambu>()
    }
}

#[async_trait]
impl Connector for BambuConnector {
    fn descriptor(&self) -> &'static Descriptor {
        &DESCRIPTOR
    }

    async fn status(&self, _ctx: &ConnCtx) -> Status {
        let st = self.bambu().status().await;
        Status { connected: st.connected, expired: st.expired, account: st.config.account_name, last_ok_at: st.last_ok_at, last_error: st.last_error }
    }

    async fn connect(&self, _ctx: &ConnCtx) -> Result<()> {
        open_login(&self.app).await
    }

    async fn disconnect(&self, _ctx: &ConnCtx) -> Result<()> {
        self.bambu().logout().await
    }

    async fn interval(&self, _ctx: &ConnCtx, cap: Cap) -> Option<u64> {
        if cap == Cap::PrintersRead && self.bambu().printing().await {
            return Some(20);
        }
        DESCRIPTOR.interval(cap)
    }

    async fn heartbeat(&self, _ctx: &ConnCtx) -> Result<SyncOutcome> {
        Ok(SyncOutcome { messages: self.bambu().check_login().await, changed: false })
    }

    async fn sync(&self, _ctx: &ConnCtx, cap: Cap) -> Result<SyncOutcome> {
        let app = &self.app;
        match cap {
            Cap::SpoolsRead => {
                let mut out = SyncOutcome::changed(sync_messages(&sync_filament(app).await?));
                // wartende Rest-Änderungen auch ohne Druckerstatus nachholen
                if !self.bambu().printing().await {
                    out.messages.extend(filament_write::process_pending(app).await);
                }
                Ok(out)
            }
            Cap::PrintersRead => {
                let m = printers::poll(app).await?;
                let mut out = SyncOutcome { changed: !m.is_empty(), messages: m };
                // Änderungen, die während eines Drucks warten mussten, jetzt ins Konto schreiben
                if !self.bambu().printing().await {
                    let w = filament_write::process_pending(app).await;
                    out.changed |= !w.is_empty();
                    out.messages.extend(w);
                }
                Ok(out)
            }
            Cap::PrintHistoryRead => {
                let mut out = SyncOutcome::default();
                if let Ok(m) = printers::match_finished(app).await {
                    out.changed |= !m.is_empty();
                    out.messages.extend(m);
                }
                let full = !self.bambu().history_full_done.swap(true, Ordering::SeqCst);
                let r = history::sync(app, full).await?;
                out.changed |= r.added > 0 || r.matched > 0;
                Ok(out)
            }
            Cap::CatalogRead => {
                let r = makerworld::sync(app).await?;
                Ok(SyncOutcome::changed(makerworld::sync_messages(&r)))
            }
            _ => Ok(SyncOutcome::default()),
        }
    }
}

// ---------- Übernahme alter Einstellungen (bis 0.3) ----------

/// Einmalig: `bambu.json` in `connectors.json` übernehmen. Ohne alte Datei gelten die bisherigen
/// Werkseinstellungen (Filament, Drucker, MakerWorld an; Schreiben aus). Die alte Datei bleibt liegen.
pub fn migrate_config(dir: &Path) -> Result<()> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", default)]
    struct Legacy {
        region: String,
        account_name: Option<String>,
        uid: Option<i64>,
        expires_at: Option<String>,
        filament: bool,
        printers: bool,
        makerworld: bool,
        filament_write: bool,
    }
    impl Default for Legacy {
        fn default() -> Self {
            Self { region: "global".into(), account_name: None, uid: None, expires_at: None, filament: true, printers: true, makerworld: true, filament_write: false }
        }
    }
    let mut cfg = ConnectorsConfig::load(dir);
    if cfg.connectors.contains_key(ID) {
        return Ok(());
    }
    let l: Legacy = std::fs::read_to_string(dir.join(LEGACY_CONFIG)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    let e = cfg.entry(ID);
    e.enabled = true;
    e.caps.insert(Cap::SpoolsRead, l.filament);
    e.caps.insert(Cap::PrintersRead, l.printers);
    e.caps.insert(Cap::PrintHistoryRead, l.printers);
    e.caps.insert(Cap::CatalogRead, l.makerworld);
    e.caps.insert(Cap::SpoolsWrite, l.filament_write);
    e.options = serde_json::to_value(Account { region: l.region, account_name: l.account_name, uid: l.uid, expires_at: l.expires_at }).unwrap_or_default();
    cfg.save(dir)
}

/// Einmalig: Token vom alten Schlüsselbund-Eintrag `bambu-token` nach `bambu-cloud:token` kopieren.
/// Der alte Eintrag bleibt, damit ein Zurückwechseln auf 0.3 angemeldet bleibt; „Abmelden“ löscht beide.
pub fn migrate_token(legacy: &dyn SecretBackend, scope: &SecretScope) -> Result<()> {
    if scope.get("token")?.is_some() {
        return Ok(());
    }
    if let Some(t) = legacy.get(LEGACY_TOKEN_ACCOUNT)?.filter(|t| !t.is_empty()) {
        scope.set("token", &t)?;
    }
    Ok(())
}

// ---------- Filament ----------

/// Alle Seiten der Spulenliste holen (das Konto liefert sonst nur die erste Seite).
pub(crate) async fn fetch_all_spools(b: &Bambu) -> Result<serde_json::Value> {
    const PAGE: usize = 50;
    let mut hits = Vec::new();
    let mut offset = 0;
    loop {
        let v = b.get(&format!("{FILAMENT_PATH}?offset={offset}&limit={PAGE}")).await?;
        let page = v["hits"].as_array().ok_or_else(|| AppError::Invalid("Filament aus dem Konto: Liste „hits“ fehlt".into()))?;
        let n = page.len();
        hits.extend(page.iter().cloned());
        offset += PAGE;
        let total = v["total"].as_u64().map(|t| t as usize);
        if n < PAGE || total.is_some_and(|t| offset >= t) || offset >= 2000 {
            break;
        }
    }
    Ok(serde_json::json!({ "hits": hits }))
}

/// Spulen aus dem Konto holen und abgleichen. Eine leere Liste wird nicht übernommen
/// (sonst wären bei einer Störung alle Spulen „nicht mehr vorhanden“).
pub async fn sync_filament(app: &AppHandle) -> Result<SyncResult> {
    let bambu = app.state::<Bambu>();
    let v = fetch_all_spools(&bambu).await?;
    let list = crate::filament::parse_cloud(&v)?;
    let mut r = if list.is_empty() {
        SyncResult { found: true, source: "konto".into(), ..Default::default() }
    } else {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.store.sync_spools(&list).await?
    };
    r.path = "Bambu-Konto".into();
    bambu.status.write().await.filament_synced_at = Some(crate::store::now());
    Ok(r)
}

fn sync_messages(r: &SyncResult) -> Vec<String> {
    let mut m = Vec::new();
    if r.added > 0 {
        m.push(format!("{} neue Spulen aus dem Bambu-Konto", r.added));
    }
    if r.updated > 0 {
        m.push(format!("{} Spulen im Bambu-Konto geändert", r.updated));
    }
    m
}

/// Knopf „Abgleichen“ und Öffnen der Filament-Seite: Konto, falls aktiv, sonst spools.json.
/// Ohne `force` wird das Konto höchstens alle 2 Minuten gefragt.
#[tauri::command]
pub async fn filament_sync(app: AppHandle, force: bool) -> Result<SyncResult> {
    let bambu = app.state::<Bambu>();
    if bambu.filament_active().await {
        let fresh = bambu
            .status
            .read()
            .await
            .filament_synced_at
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .is_some_and(|t| (chrono::Utc::now() - t.to_utc()).num_seconds() < FILAMENT_FRESH_SECS);
        if fresh && !force {
            return Ok(SyncResult { found: true, source: "konto".into(), path: "Bambu-Konto".into(), ..Default::default() });
        }
        return sync_filament(&app).await;
    }
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let file = state.spools_file(&ctx);
    let mut r = crate::watch::sync_spools(&ctx, &file).await?;
    r.found = file.is_file();
    r.source = "datei".into();
    Ok(r)
}

/// Ablaufdatum aus dem Token lesen (JWT `exp`), sonst heute + 90 Tage.
fn token_expiry(token: &str) -> String {
    let exp = token
        .split('.')
        .nth(1)
        .and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("exp")?.as_i64())
        .and_then(|s| chrono::DateTime::from_timestamp(s, 0));
    exp.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::days(DEFAULT_DAYS))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ---------- Anmeldung ----------

/// Anmeldefenster von Bambu öffnen. Es ist privat (kein gespeicherter Verlauf) und hat keinen Zugriff
/// auf die Befehle der App. Sobald das Anmelde-Cookie da ist, übernimmt die App das Token und schließt das Fenster.
async fn open_login(app: &AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window(LOGIN_LABEL) {
        let _ = w.set_focus();
        return Ok(());
    }
    let url = LOGIN_URL.parse().map_err(|_| AppError::Invalid("Anmeldeadresse ungültig".into()))?;
    let w = WebviewWindowBuilder::new(app, LOGIN_LABEL, WebviewUrl::External(url))
        .title("Bei Bambu Lab anmelden")
        .inner_size(520.0, 760.0)
        .incognito(true)
        .build()
        .map_err(|e| AppError::Invalid(e.to_string()))?;
    app.state::<Bambu>().status.write().await.logging_in = true;
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let bambu = app2.state::<Bambu>();
        let mut found = None;
        // bis zu 15 Minuten warten, solange das Fenster offen ist
        for _ in 0..600 {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            let Some(win) = app2.get_webview_window(LOGIN_LABEL) else { break };
            if let Ok(cookies) = win.cookies() {
                if let Some(c) = cookies.iter().find(|c| c.name() == "token" && c.value().len() > 20) {
                    found = Some(c.value().to_string());
                    let _ = win.close();
                    break;
                }
            }
        }
        bambu.status.write().await.logging_in = false;
        let messages = match found {
            Some(t) => match bambu.accept_token(t).await {
                Ok(name) => {
                    let mut m = vec![format!("Mit dem Bambu-Konto verbunden: {name}")];
                    if bambu.filament_active().await {
                        if let Ok(r) = sync_filament(&app2).await {
                            m.extend(sync_messages(&r));
                        }
                    }
                    m
                }
                Err(e) => vec![format!("Anmeldung nicht übernommen: {e}")],
            },
            None => vec![],
        };
        let _ = app2.emit(EVENT, Changed { messages });
    });
    drop(w);
    Ok(())
}

// ---------- Befehle (Oberfläche unverändert; ab Stufe A6 generisch über „Anbindungen“) ----------

#[tauri::command]
pub async fn bambu_status(bambu: State<'_, Bambu>) -> Result<BambuStatus> {
    Ok(bambu.status().await)
}

#[tauri::command]
pub async fn bambu_login(app: AppHandle) -> Result<()> {
    open_login(&app).await
}

#[tauri::command]
pub async fn bambu_logout(bambu: State<'_, Bambu>) -> Result<()> {
    bambu.logout().await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BambuOptions {
    filament: bool,
    printers: bool,
    makerworld: bool,
    #[serde(default)]
    filament_write: bool,
}

#[tauri::command]
pub async fn bambu_set_options(registry: State<'_, Arc<Registry>>, options: BambuOptions) -> Result<()> {
    registry
        .set_caps(
            ID,
            &[
                (Cap::SpoolsRead, options.filament),
                (Cap::PrintersRead, options.printers),
                (Cap::PrintHistoryRead, options.printers),
                (Cap::CatalogRead, options.makerworld),
                (Cap::SpoolsWrite, options.filament_write),
            ],
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::secrets::MemoryBackend;

    #[test]
    fn schreiben_nur_fuer_spulen() {
        assert_eq!(DESCRIPTOR.write_paths, &[FILAMENT_PATH, FILAMENT_BATCH_PATH]);
        assert_eq!(FILAMENT_BATCH_PATH, format!("{FILAMENT_PATH}/batch"));
        assert!(DESCRIPTOR.caps.iter().filter(|c| c.is_write()).eq([Cap::SpoolsWrite].iter()));
    }

    #[test]
    fn ablauf_aus_token() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"exp":1800000000}"#);
        assert_eq!(token_expiry(&format!("x.{payload}.y")), "2027-01-15T08:00:00Z");
        // kein JWT: heute + 90 Tage
        let d = chrono::DateTime::parse_from_rfc3339(&token_expiry("undurchsichtig")).unwrap();
        let days = (d.to_utc() - chrono::Utc::now()).num_days();
        assert!((89..=90).contains(&days));
    }

    #[test]
    fn alte_einstellungen_uebernehmen() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(LEGACY_CONFIG),
            r#"{"region":"global","accountName":"Test","uid":7,"filament":true,"printers":false,"makerworld":true,"filamentWrite":true}"#,
        )
        .unwrap();
        migrate_config(dir.path()).unwrap();
        let cfg = ConnectorsConfig::load(dir.path());
        assert!(cfg.enabled(ID));
        assert!(cfg.cap_on(&DESCRIPTOR, Cap::SpoolsRead));
        assert!(!cfg.cap_on(&DESCRIPTOR, Cap::PrintersRead));
        assert!(!cfg.cap_on(&DESCRIPTOR, Cap::PrintHistoryRead), "Verlauf folgt dem Drucker-Schalter");
        assert!(cfg.cap_on(&DESCRIPTOR, Cap::SpoolsWrite));
        assert_eq!(cfg.get(ID).options["accountName"], "Test");
        assert!(dir.path().join(LEGACY_CONFIG).exists(), "alte Datei bleibt liegen");
        let s = std::fs::read_to_string(dir.path().join(crate::connectors::config::FILE)).unwrap();
        assert!(!s.to_lowercase().contains("token\""), "kein Token in connectors.json");

        // zweiter Lauf ändert nichts mehr, auch wenn bambu.json sich ändert
        std::fs::write(dir.path().join(LEGACY_CONFIG), r#"{"filament":false}"#).unwrap();
        migrate_config(dir.path()).unwrap();
        assert!(ConnectorsConfig::load(dir.path()).cap_on(&DESCRIPTOR, Cap::SpoolsRead));
    }

    #[test]
    fn ohne_alte_datei_bisherige_werkseinstellungen() {
        let dir = tempfile::tempdir().unwrap();
        migrate_config(dir.path()).unwrap();
        let cfg = ConnectorsConfig::load(dir.path());
        for c in [Cap::SpoolsRead, Cap::PrintersRead, Cap::PrintHistoryRead, Cap::CatalogRead] {
            assert!(cfg.cap_on(&DESCRIPTOR, c), "{c:?}");
        }
        assert!(!cfg.cap_on(&DESCRIPTOR, Cap::SpoolsWrite));
    }

    #[test]
    fn token_umziehen() {
        let backend: Arc<MemoryBackend> = Arc::new(MemoryBackend::default());
        let scope = SecretScope::new(backend.clone(), ID, DESCRIPTOR.secrets);
        // nichts da: nichts passiert
        migrate_token(backend.as_ref(), &scope).unwrap();
        assert_eq!(scope.get("token").unwrap(), None);
        // alter Eintrag wird kopiert und bleibt für ein Zurückwechseln erhalten
        backend.set(LEGACY_TOKEN_ACCOUNT, "abc").unwrap();
        migrate_token(backend.as_ref(), &scope).unwrap();
        assert_eq!(backend.get("bambu-cloud:token").unwrap().as_deref(), Some("abc"));
        assert_eq!(backend.get(LEGACY_TOKEN_ACCOUNT).unwrap().as_deref(), Some("abc"));
        // neuer Eintrag hat Vorrang vor einem späteren alten
        backend.set(LEGACY_TOKEN_ACCOUNT, "alt").unwrap();
        migrate_token(backend.as_ref(), &scope).unwrap();
        assert_eq!(scope.get("token").unwrap().as_deref(), Some("abc"));
    }
}
