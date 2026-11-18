//! Apple Kontakte am Mac (`apple-contacts`): liest über das Contacts-Framework alle am Mac eingerichteten
//! Konten (iCloud, Exchange, Google …). Nur lesen; Notizen werden nicht gelesen (brauchen ein eigenes Entitlement).
//! Die Freigabe erteilt der Nutzer im Dialog von macOS („Datenschutz & Sicherheit → Kontakte“).

use crate::connectors::contacts::{normalize_kind, ContactSource, ExtContact, PostalAddress, Typed};
use crate::connectors::{AuthKind, Cap, ConnCtx, Connector, Descriptor, Kind, Platform, Status, SyncOutcome};
use crate::error::{AppError, Result};
use async_trait::async_trait;
use block2::{RcBlock, StackBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::AnyThread;
use objc2_contacts::{
    CNAuthorizationStatus, CNContact, CNContactEmailAddressesKey, CNContactFamilyNameKey, CNContactFetchRequest,
    CNContactGivenNameKey, CNContactIdentifierKey, CNContactOrganizationNameKey, CNContactPhoneNumbersKey,
    CNContactPostalAddressesKey, CNContactStore, CNEntityType, CNKeyDescriptor,
};
use objc2_foundation::{NSArray, NSError, NSString};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::time::Duration;
use tauri::Manager;

pub static DESCRIPTOR: Descriptor = Descriptor {
    id: "apple-contacts",
    name: "Apple Kontakte (dieser Mac)",
    kind: Kind::Contacts,
    platforms: &[Platform::MacOs],
    auth: AuthKind::OsPermission,
    caps: &[Cap::ContactsRead],
    base_url: "",
    write_paths: &[],
    read_post_paths: &[],
    timeout_secs: 0,
    default_enabled: false,
    min_interval_ms: 0,
    // verknüpfte Kunden alle 30 Minuten abgleichen
    intervals: &[(Cap::ContactsRead, 30 * 60)],
    secrets: &[],
    heartbeat_secs: 0,
};

pub struct AppleContacts {
    pub app: tauri::AppHandle,
}

fn authorized() -> bool {
    let st = unsafe { CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts) };
    st == CNAuthorizationStatus::Authorized || st == CNAuthorizationStatus::Limited
}

fn denied() -> bool {
    let st = unsafe { CNContactStore::authorizationStatusForEntityType(CNEntityType::Contacts) };
    st == CNAuthorizationStatus::Denied || st == CNAuthorizationStatus::Restricted
}

/// Freigabe anfragen; wartet auf die Antwort im Dialog von macOS.
fn request_access() -> Result<bool> {
    let store = unsafe { CNContactStore::new() };
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
        let _ = tx.send(granted.as_bool());
    });
    unsafe { store.requestAccessForEntityType_completionHandler(CNEntityType::Contacts, &block) };
    rx.recv_timeout(Duration::from_secs(600)).map_err(|_| AppError::Invalid("Keine Antwort auf die Freigabe der Kontakte.".into()))
}

fn s(x: &NSString) -> String {
    x.to_string()
}

fn label(l: Option<Retained<NSString>>) -> String {
    l.map(|x| normalize_kind(&x.to_string())).unwrap_or_default()
}

fn convert(c: &CNContact) -> ExtContact {
    unsafe {
        let emails = c.emailAddresses().iter().map(|lv| Typed { kind: label(lv.label()), value: s(&lv.value()) }).collect();
        let phones = c.phoneNumbers().iter().map(|lv| Typed { kind: label(lv.label()), value: s(&lv.value().stringValue()) }).collect();
        let addresses = c
            .postalAddresses()
            .iter()
            .map(|lv| {
                let a = lv.value();
                Typed {
                    kind: label(lv.label()),
                    value: PostalAddress { street: s(&a.street()), zip: s(&a.postalCode()), city: s(&a.city()), country: s(&a.ISOCountryCode()) },
                }
            })
            .collect();
        let given = s(&c.givenName());
        let family = s(&c.familyName());
        ExtContact {
            external_id: s(&c.identifier()),
            display_name: format!("{given} {family}").trim().to_string(),
            given_name: given,
            family_name: family,
            organization: s(&c.organizationName()),
            emails,
            phones,
            addresses,
            ..Default::default()
        }
    }
}

/// Alle Kontakte lesen (blockierend; auf einem Hintergrund-Thread aufrufen).
fn fetch_all() -> Result<Vec<ExtContact>> {
    if !authorized() {
        return Err(AppError::Invalid("Printmanager hat keinen Zugriff auf die Kontakte. Bitte unter Anbindungen freigeben.".into()));
    }
    let store = unsafe { CNContactStore::new() };
    let keys: Vec<&ProtocolObject<dyn CNKeyDescriptor>> = unsafe {
        [
            CNContactIdentifierKey,
            CNContactGivenNameKey,
            CNContactFamilyNameKey,
            CNContactOrganizationNameKey,
            CNContactEmailAddressesKey,
            CNContactPhoneNumbersKey,
            CNContactPostalAddressesKey,
        ]
    }
    .into_iter()
    .map(ProtocolObject::from_ref)
    .collect();
    let keys = NSArray::from_slice(&keys);
    let request = unsafe { CNContactFetchRequest::initWithKeysToFetch(CNContactFetchRequest::alloc(), &keys) };
    let out = RefCell::new(Vec::new());
    let block = StackBlock::new(|c: NonNull<CNContact>, _stop: NonNull<Bool>| {
        out.borrow_mut().push(convert(unsafe { c.as_ref() }));
    });
    let mut err: Option<Retained<NSError>> = None;
    let ok = unsafe { store.enumerateContactsWithFetchRequest_error_usingBlock(&request, Some(&mut err), &block) };
    if !ok {
        let msg = err.map(|e| e.localizedDescription().to_string()).unwrap_or_default();
        return Err(AppError::Invalid(format!("Kontakte nicht lesbar: {msg}")));
    }
    Ok(out.into_inner())
}

#[async_trait]
impl Connector for AppleContacts {
    fn descriptor(&self) -> &'static Descriptor {
        &DESCRIPTOR
    }

    async fn status(&self, _ctx: &ConnCtx) -> Status {
        let ok = authorized();
        Status {
            connected: ok,
            detail: if denied() { Some("in den Systemeinstellungen unter Datenschutz → Kontakte erlauben".into()) } else { None },
            ..Default::default()
        }
    }

    /// Freigabe im Dialog von macOS anfragen und die Anbindung einschalten.
    async fn connect(&self, _ctx: &ConnCtx) -> Result<()> {
        if denied() {
            return Err(AppError::Invalid(
                "Der Zugriff auf die Kontakte wurde abgelehnt. Erlaube ihn unter Systemeinstellungen → Datenschutz & Sicherheit → Kontakte.".into(),
            ));
        }
        let granted = tokio::task::spawn_blocking(request_access).await.map_err(|e| AppError::Invalid(e.to_string()))??;
        if granted {
            Ok(())
        } else {
            Err(AppError::Invalid("Zugriff auf die Kontakte nicht erteilt.".into()))
        }
    }

    /// Die Freigabe nimmt nur macOS zurück; hier wird die Anbindung nur ausgeschaltet.
    async fn disconnect(&self, _ctx: &ConnCtx) -> Result<()> {
        Ok(())
    }

    /// Verknüpfte Kunden mit den Kontakten abgleichen (nur lesen; die App schreibt nie ins Adressbuch).
    async fn sync(&self, ctx: &ConnCtx, cap: Cap) -> Result<SyncOutcome> {
        if cap != Cap::ContactsRead {
            return Ok(SyncOutcome::default());
        }
        let list = self.list_all(ctx).await?;
        let state = self.app.state::<crate::commands::AppState>();
        let c = state.ctx.read().await;
        let r = crate::contactsync::sync(&c.store, DESCRIPTOR.id, &list).await?;
        Ok(SyncOutcome {
            changed: !(r.updated.is_empty() && r.conflicts.is_empty() && r.gone.is_empty()),
            messages: r.messages(DESCRIPTOR.name),
        })
    }

    fn contacts(&self) -> Option<&dyn ContactSource> {
        Some(self)
    }
}

#[async_trait]
impl ContactSource for AppleContacts {
    async fn list_all(&self, _ctx: &ConnCtx) -> Result<Vec<ExtContact>> {
        tokio::task::spawn_blocking(fetch_all).await.map_err(|e| AppError::Invalid(e.to_string()))?
    }
}
