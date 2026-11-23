//! Geheimnisse (Tokens, Passwörter) nur im Schlüsselbund des Systems (AGENTS.md Regel 4).
//! Dienst = Kennung der App (`de.printmanager.app`, Entwicklerversion `de.printmanager.app.dev`),
//! Konto `<anbindung>:<name>`, z.B. `google-contacts:refresh`.
//! In Tests ersetzt `MemoryBackend` den Schlüsselbund.

use crate::error::{AppError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Dienst im Schlüsselbund; ab Werk die Kennung der stabilen App
pub const DEFAULT_SERVICE: &str = "de.printmanager.app";
static SERVICE: OnceLock<String> = OnceLock::new();

/// Dienst auf die Kennung der laufenden App setzen (einmal beim Start), damit die Entwicklerversion
/// eigene Einträge hat und nie das Token der stabilen App liest oder löscht.
pub fn set_service(identifier: &str) {
    let _ = SERVICE.set(identifier.to_string());
}

pub fn service() -> &'static str {
    SERVICE.get().map(String::as_str).unwrap_or(DEFAULT_SERVICE)
}

pub trait SecretBackend: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> Result<()>;
    /// Löschen; ein fehlender Eintrag ist kein Fehler.
    fn delete(&self, account: &str) -> Result<()>;
}

/// Schlüsselbund des Betriebssystems (macOS Keychain, Windows Anmeldeinformationen)
pub struct KeyringBackend;

fn entry(account: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(service(), account).map_err(|e| AppError::Invalid(format!("Schlüsselbund: {e}")))
}

impl SecretBackend for KeyringBackend {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match entry(account)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Invalid(format!("Schlüsselbund: {e}"))),
        }
    }
    fn set(&self, account: &str, value: &str) -> Result<()> {
        entry(account)?.set_password(value).map_err(|e| AppError::Invalid(format!("Schlüsselbund: {e}")))
    }
    fn delete(&self, account: &str) -> Result<()> {
        match entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Invalid(format!("Schlüsselbund: {e}"))),
        }
    }
}

/// Speicher-Ersatz für Tests (nie im Programm verwenden)
#[derive(Default)]
pub struct MemoryBackend(Mutex<HashMap<String, String>>);

impl SecretBackend for MemoryBackend {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).get(account).cloned())
    }
    fn set(&self, account: &str, value: &str) -> Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(account.into(), value.into());
        Ok(())
    }
    fn delete(&self, account: &str) -> Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(account);
        Ok(())
    }
}

/// Geheimnisse einer einzelnen Anbindung; sie sieht nur ihre eigenen Einträge.
#[derive(Clone)]
pub struct SecretScope {
    backend: Arc<dyn SecretBackend>,
    connector: &'static str,
    names: &'static [&'static str],
}

impl SecretScope {
    pub fn new(backend: Arc<dyn SecretBackend>, connector: &'static str, names: &'static [&'static str]) -> Self {
        Self { backend, connector, names }
    }

    pub fn account(&self, name: &str) -> String {
        format!("{}:{name}", self.connector)
    }

    /// Nur Namen aus dem Descriptor sind erlaubt, damit „Trennen“ alles findet.
    fn check(&self, name: &str) -> Result<()> {
        if self.names.contains(&name) {
            Ok(())
        } else {
            Err(AppError::Invalid(format!("Unbekanntes Geheimnis „{name}“ für {}", self.connector)))
        }
    }

    pub fn get(&self, name: &str) -> Result<Option<String>> {
        self.check(name)?;
        self.backend.get(&self.account(name))
    }

    pub fn set(&self, name: &str, value: &str) -> Result<()> {
        self.check(name)?;
        self.backend.set(&self.account(name), value)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        self.check(name)?;
        self.backend.delete(&self.account(name))
    }

    /// Alle Geheimnisse der Anbindung löschen (beim Trennen).
    pub fn clear(&self) -> Result<()> {
        for n in self.names {
            self.backend.delete(&self.account(n))?;
        }
        Ok(())
    }
}
