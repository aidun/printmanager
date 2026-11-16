//! Bambu-Konto: Anmeldung im Anmeldefenster von Bambu, Token im Schlüsselbund des Systems,
//! gedrosselter Cloud-Zugang und ein Hintergrund-Takt. Alles nur lesend.
//! Das Passwort sieht die App nie; das Token steht nie in Datenbank, Einstellungen oder Logs.

pub mod cloud;
pub mod makerworld;
pub mod printers;

use crate::commands::AppState;
use crate::error::{AppError, Result};
use crate::model::SyncResult;
use crate::watch::{Changed, EVENT};
use base64::Engine;
use cloud::{Cloud, CloudError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::RwLock;

const LOGIN_LABEL: &str = "bambu-login";
const LOGIN_URL: &str = "https://bambulab.com/en/sign-in";
const KEYRING_SERVICE: &str = "de.printmanager.app";
const KEYRING_USER: &str = "bambu-token";
/// Gültigkeit, falls das Token kein Ablaufdatum verrät (laut Community ca. 90 Tage)
const DEFAULT_DAYS: i64 = 90;
/// Takt: Grundschritt, Filament-Abgleich und Anmeldeprüfung (in Grundschritten)
const STEP: Duration = Duration::from_secs(20);
const FILAMENT_EVERY: u64 = 45; // 15 Minuten
const HEALTH_EVERY: u64 = 90; // 30 Minuten
const PRINTERS_EVERY: u64 = 3; // 1 Minute, während eines Drucks bei jedem Schritt (20 s)
const HISTORY_EVERY: u64 = 45; // Druckverlauf alle 15 Minuten
const MAKERWORLD_EVERY: u64 = 1080; // MakerWorld alle 6 Stunden (erstmals im zweiten Schritt)
/// „Jetzt abgleichen“ beim Öffnen der Filament-Seite höchstens so oft
const FILAMENT_FRESH_SECS: i64 = 120;
const FILAMENT_PATH: &str = "/v1/design-user-service/my/filament/v2";

/// Nicht geheime Angaben zum Konto und die Schalter, gespeichert in `bambu.json` im Konfigurationsordner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BambuConfig {
    pub region: String,
    pub account_name: Option<String>,
    pub uid: Option<i64>,
    pub expires_at: Option<String>,
    /// Filament aus dem Konto (Stufe 2)
    pub filament: bool,
    /// Druckerstatus (Stufe 3)
    pub printers: bool,
    /// MakerWorld im Katalog (Stufe 4)
    pub makerworld: bool,
}

impl Default for BambuConfig {
    fn default() -> Self {
        Self {
            region: "global".into(),
            account_name: None,
            uid: None,
            expires_at: None,
            filament: true,
            printers: true,
            makerworld: true,
        }
    }
}

impl BambuConfig {
    fn load(file: &std::path::Path) -> Self {
        std::fs::read_to_string(file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    fn save(&self, file: &std::path::Path) -> Result<()> {
        if let Some(d) = file.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(file, serde_json::to_string_pretty(self).unwrap_or_default())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BambuStatus {
    #[serde(flatten)]
    pub config: BambuConfig,
    /// Token vorhanden und nicht als abgelaufen bekannt
    pub connected: bool,
    pub expired: bool,
    pub last_ok_at: Option<String>,
    pub last_error: Option<String>,
    /// Anmeldefenster ist gerade offen
    pub logging_in: bool,
    /// letzter erfolgreicher Filament-Abgleich mit dem Konto
    pub filament_synced_at: Option<String>,
    /// letzter MakerWorld-Abgleich
    pub mw_synced_at: Option<String>,
}

pub struct Bambu {
    config_file: PathBuf,
    pub cloud: Cloud,
    token: RwLock<Option<String>>,
    status: RwLock<BambuStatus>,
    /// zuletzt gesehene Drucker (nur im Speicher)
    pub printers: RwLock<Vec<printers::PrinterView>>,
    pub printers_at: RwLock<Option<String>>,
}

impl Bambu {
    pub fn new(config_dir: &std::path::Path) -> Self {
        let config_file = config_dir.join("bambu.json");
        let config = BambuConfig::load(&config_file);
        let token = token_load();
        let status = BambuStatus { connected: token.is_some(), config, ..Default::default() };
        Self {
            config_file,
            cloud: Cloud::new(cloud::GLOBAL, Duration::from_millis(1000)),
            token: RwLock::new(token),
            status: RwLock::new(status),
            printers: RwLock::new(Vec::new()),
            printers_at: RwLock::new(None),
        }
    }

    pub async fn status(&self) -> BambuStatus {
        self.status.read().await.clone()
    }

    /// GET mit dem gespeicherten Token. Merkt sich Erfolg, Fehler und Ablauf.
    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let token = self.token.read().await.clone().ok_or_else(|| AppError::Invalid("Nicht am Bambu-Konto angemeldet.".into()))?;
        let r = self.cloud.get(&token, path).await;
        let mut st = self.status.write().await;
        match &r {
            Ok(_) => {
                st.last_ok_at = Some(crate::store::now());
                st.last_error = None;
                st.expired = false;
            }
            Err(CloudError::Expired) => {
                st.expired = true;
                st.connected = false;
                st.last_error = Some(CloudError::Expired.to_string());
            }
            Err(e) => st.last_error = Some(e.to_string()),
        }
        Ok(r?)
    }

    async fn set_config(&self, f: impl FnOnce(&mut BambuConfig)) -> Result<()> {
        let mut st = self.status.write().await;
        f(&mut st.config);
        st.config.save(&self.config_file)
    }

    /// Token übernehmen: im Schlüsselbund speichern und Kontoangaben abrufen.
    async fn accept_token(&self, token: String) -> Result<String> {
        token_save(&token)?;
        *self.token.write().await = Some(token.clone());
        {
            let mut st = self.status.write().await;
            st.connected = true;
            st.expired = false;
            st.last_error = None;
        }
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
        self.set_config(|c| {
            c.account_name = name.clone();
            c.uid = uid;
            c.expires_at = Some(expires);
        })
        .await?;
        Ok(name.unwrap_or_else(|| "Bambu-Konto".into()))
    }

    async fn logout(&self) -> Result<()> {
        token_delete()?;
        *self.token.write().await = None;
        {
            let mut st = self.status.write().await;
            st.connected = false;
            st.expired = false;
            st.last_error = None;
            st.last_ok_at = None;
        }
        self.set_config(|c| {
            c.account_name = None;
            c.uid = None;
            c.expires_at = None;
        })
        .await
    }

    /// Druckerstatus aktiv (angemeldet und Schalter an).
    pub async fn printers_active(&self) -> bool {
        let st = self.status.read().await;
        st.connected && st.config.printers
    }

    /// MakerWorld im Katalog aktiv (angemeldet und Schalter an).
    pub async fn makerworld_active(&self) -> bool {
        let st = self.status.read().await;
        st.connected && st.config.makerworld
    }

    /// Filament kommt aus dem Konto (angemeldet und Schalter an). Dann ruht der Abgleich mit spools.json.
    pub async fn filament_active(&self) -> bool {
        let st = self.status.read().await;
        st.connected && st.config.filament
    }

    /// Hintergrundprüfung: Ist die Anmeldung noch gültig? Spätere Stufen hängen hier ihre Abgleiche an.
    pub async fn tick(&self) -> Vec<String> {
        let mut messages = Vec::new();
        let was_ok = self.status.read().await.connected;
        if !was_ok {
            return messages;
        }
        if let Err(AppError::Invalid(_)) = self.get("/v1/design-user-service/my/preference").await {
            if self.status.read().await.expired {
                messages.push(CloudError::Expired.to_string());
            }
        }
        messages
    }
}

// ---------- Filament ----------

/// Spulen aus dem Konto holen und abgleichen. Eine leere Liste wird nicht übernommen
/// (sonst wären bei einer Störung alle Spulen „nicht mehr vorhanden“).
pub async fn sync_filament(app: &AppHandle) -> Result<SyncResult> {
    let bambu = app.state::<Bambu>();
    let v = bambu.get(FILAMENT_PATH).await?;
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

/// Knopf „Jetzt abgleichen“ und Öffnen der Filament-Seite: Konto, falls aktiv, sonst spools.json.
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

// ---------- Token im Schlüsselbund ----------

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| AppError::Invalid(format!("Schlüsselbund: {e}")))
}

fn token_load() -> Option<String> {
    entry().ok()?.get_password().ok().filter(|t| !t.is_empty())
}

fn token_save(t: &str) -> Result<()> {
    entry()?.set_password(t).map_err(|e| AppError::Invalid(format!("Schlüsselbund: {e}")))
}

fn token_delete() -> Result<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Invalid(format!("Schlüsselbund: {e}"))),
    }
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

// ---------- Hintergrund ----------

/// Startet den Takt: kurz nach dem Start, dann Filament alle 15 Minuten und die Anmeldeprüfung alle 30 Minuten
/// (entfällt, wenn der Filament-Abgleich ohnehin läuft).
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let mut n: u64 = 0;
        loop {
            let bambu = app.state::<Bambu>();
            let mut messages = Vec::new();
            let mut changed = false;
            if bambu.filament_active().await {
                if n.is_multiple_of(FILAMENT_EVERY) {
                    match sync_filament(&app).await {
                        Ok(r) => {
                            messages.extend(sync_messages(&r));
                            changed = true;
                        }
                        Err(e) => {
                            if bambu.status.read().await.expired {
                                messages.push(e.to_string());
                            }
                        }
                    }
                }
            } else if n.is_multiple_of(HEALTH_EVERY) && !bambu.printers_active().await {
                messages.extend(bambu.tick().await);
            }
            if bambu.printers_active().await {
                let printing = bambu.printers.read().await.iter().any(|p| printers::active(&p.state));
                if printing || n.is_multiple_of(PRINTERS_EVERY) {
                    match printers::poll(&app).await {
                        Ok(m) => {
                            if n == 0 {
                                eprintln!("Bambu-Konto: {} Drucker", bambu.printers.read().await.len());
                            }
                            changed |= !m.is_empty();
                            messages.extend(m);
                        }
                        Err(e) => {
                            eprintln!("Drucker nicht abrufbar: {e}");
                            if bambu.status.read().await.expired && !messages.iter().any(|x| x == &e.to_string()) {
                                messages.push(e.to_string());
                            }
                        }
                    }
                }
                if n.is_multiple_of(HISTORY_EVERY) {
                    if let Ok(m) = printers::match_finished(&app).await {
                        changed |= !m.is_empty();
                        messages.extend(m);
                    }
                }
            }
            if bambu.makerworld_active().await && n % MAKERWORLD_EVERY == 1 {
                match makerworld::sync(&app).await {
                    Ok(r) => {
                        messages.extend(makerworld::sync_messages(&r));
                        changed = true;
                    }
                    Err(e) => eprintln!("MakerWorld nicht abrufbar: {e}"),
                }
            }
            if changed || !messages.is_empty() {
                let _ = app.emit(EVENT, Changed { messages });
            }
            n += 1;
            tokio::time::sleep(STEP).await;
        }
    });
}

// ---------- Befehle ----------

#[tauri::command]
pub async fn bambu_status(bambu: State<'_, Bambu>) -> Result<BambuStatus> {
    Ok(bambu.status().await)
}

/// Anmeldefenster von Bambu öffnen. Es ist privat (kein gespeicherter Verlauf) und hat keinen Zugriff
/// auf die Befehle der App. Sobald das Anmelde-Cookie da ist, übernimmt die App das Token und schließt das Fenster.
#[tauri::command]
pub async fn bambu_login(app: AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window(LOGIN_LABEL) {
        let _ = w.set_focus();
        return Ok(());
    }
    let w = WebviewWindowBuilder::new(&app, LOGIN_LABEL, WebviewUrl::External(LOGIN_URL.parse().unwrap()))
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
}

#[tauri::command]
pub async fn bambu_set_options(bambu: State<'_, Bambu>, options: BambuOptions) -> Result<()> {
    bambu
        .set_config(|c| {
            c.filament = options.filament;
            c.printers = options.printers;
            c.makerworld = options.makerworld;
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn einstellungen_ohne_token() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("bambu.json");
        let mut c = BambuConfig::load(&f);
        assert_eq!(c.region, "global");
        assert!(c.filament && c.printers && c.makerworld);
        c.account_name = Some("Test".into());
        c.save(&f).unwrap();
        let s = std::fs::read_to_string(&f).unwrap();
        assert!(!s.to_lowercase().contains("token"), "kein Token in bambu.json");
        assert_eq!(BambuConfig::load(&f).account_name.as_deref(), Some("Test"));
    }
}
