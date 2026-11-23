//! Kunden aus Kontakten übernehmen, verknüpfen und abgleichen (docs/PLAN-Anbindungen.md, K1/K2).
//!
//! - Nur lesen: Die App schreibt nie in ein Adressbuch.
//! - Nur verknüpfte Kontakte werden abgeglichen; das Adressbuch wird nicht in die Kundenliste kopiert.
//! - Abgleich je Feld mit dem letzten gemeinsamen Stand beider Seiten (Drei-Wege-Vergleich):
//!   nur im Kontakt geändert → übernehmen; nur beim Kunden geändert → bleibt; beide → Konflikt, der Nutzer entscheidet.
//! - Notiz und Rechnungsadresse fasst der Abgleich nie an.

use crate::connectors::contacts::{ExtContact, PostalAddress, Typed};
use crate::connectors::registry::Registry;
use crate::connectors::Cap;
use crate::error::{AppError, Result};
use crate::model::{ExternalLink, LinkInput, Person, PersonInput};
use crate::store::Store;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock};

pub const ENTITY: &str = "person";

/// Felder, die ein Kontakt beim Kunden bestimmt
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Fields {
    pub name: String,
    pub email: String,
    pub phone: String,
    pub street: String,
    pub house_no: String,
    pub zip: String,
    pub city: String,
}

pub const FIELD_NAMES: [&str; 7] = ["name", "email", "phone", "street", "houseNo", "zip", "city"];

/// Anzeigename eines Feldes
pub fn field_label(f: &str) -> &'static str {
    match f {
        "name" => "Name",
        "email" => "E-Mail",
        "phone" => "Telefon",
        "street" => "Straße",
        "houseNo" => "Hausnr.",
        "zip" => "PLZ",
        "city" => "Ort",
        _ => "Feld",
    }
}

impl Fields {
    fn get(&self, f: &str) -> &str {
        match f {
            "name" => &self.name,
            "email" => &self.email,
            "phone" => &self.phone,
            "street" => &self.street,
            "houseNo" => &self.house_no,
            "zip" => &self.zip,
            "city" => &self.city,
            _ => "",
        }
    }

    fn set(&mut self, f: &str, v: String) {
        match f {
            "name" => self.name = v,
            "email" => self.email = v,
            "phone" => self.phone = v,
            "street" => self.street = v,
            "houseNo" => self.house_no = v,
            "zip" => self.zip = v,
            "city" => self.city = v,
            _ => {}
        }
    }

    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }

    pub fn of_person(p: &Person) -> Self {
        Self {
            name: p.name.clone(),
            email: p.email.clone(),
            phone: p.phone.clone(),
            street: p.street.clone(),
            house_no: p.house_no.clone(),
            zip: p.zip.clone(),
            city: p.city.clone(),
        }
    }
}

static RE_NR_LAST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?\D)\s+(\d+\s*[a-zA-Z]?(?:\s*[-/]\s*\d+\s*[a-zA-Z]?)?)$").unwrap());
static RE_NR_FIRST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+[a-zA-Z]?)\s+(\D.*)$").unwrap());

/// Straße und Hausnummer trennen: „Musterstraße 12a“ → („Musterstraße“, „12a“), „12 Main St“ → („Main St“, „12“).
/// Weitere Zeilen (Zusatz) bleiben an der Straße.
pub fn split_street(s: &str) -> (String, String) {
    let mut lines = s.lines();
    let first = lines.next().unwrap_or("").trim();
    let rest: Vec<&str> = lines.map(str::trim).filter(|l| !l.is_empty()).collect();
    let (street, nr) = if let Some(c) = RE_NR_LAST.captures(first) {
        (c[1].trim().to_string(), c[2].split_whitespace().collect::<String>())
    } else if let Some(c) = RE_NR_FIRST.captures(first) {
        (c[2].trim().to_string(), c[1].to_string())
    } else {
        (first.to_string(), String::new())
    };
    let street = std::iter::once(street).chain(rest.into_iter().map(str::to_string)).collect::<Vec<_>>().join(", ");
    (street, nr)
}

/// Bevorzugter Eintrag: erst die gewünschten Arten in Reihenfolge, sonst der erste.
fn pick<'a, T>(list: &'a [Typed<T>], prefer: &[&str]) -> Option<&'a T> {
    prefer.iter().find_map(|k| list.iter().find(|x| x.kind == *k)).or_else(|| list.first()).map(|x| &x.value)
}

/// Kontakt → Kundenfelder. Name: Personenname, sonst Firma. Telefon: Mobil vor Privat. Adresse: Privat zuerst.
pub fn from_contact(c: &ExtContact) -> Fields {
    let person = format!("{} {}", c.given_name.trim(), c.family_name.trim()).trim().to_string();
    let name = if !person.is_empty() {
        person
    } else if !c.display_name.trim().is_empty() {
        c.display_name.trim().to_string()
    } else {
        c.organization.trim().to_string()
    };
    let addr = pick(&c.addresses, &["privat", "arbeit"]).cloned().unwrap_or_else(PostalAddress::default);
    let (street, house_no) = split_street(&addr.street);
    Fields {
        name,
        email: pick(&c.emails, &["privat", "arbeit"]).map(|x| x.trim().to_string()).unwrap_or_default(),
        phone: pick(&c.phones, &["mobil", "privat", "haupt"]).map(|x| x.trim().to_string()).unwrap_or_default(),
        street,
        house_no,
        zip: addr.zip.trim().to_string(),
        city: addr.city.trim().to_string(),
    }
}

/// Ergebnis des Vergleichs je Feld
#[derive(Debug, Default, PartialEq)]
pub struct Merge {
    /// aus dem Kontakt zu übernehmen
    pub take: Vec<(&'static str, String)>,
    /// auf beiden Seiten verschieden geändert
    pub conflicts: Vec<&'static str>,
}

/// Drei-Wege-Vergleich je Feld: `base_*` ist der letzte gemeinsame Stand, `remote`/`local` der jetzige.
pub fn merge(base_remote: &Fields, base_local: &Fields, remote: &Fields, local: &Fields) -> Merge {
    let mut m = Merge::default();
    for f in FIELD_NAMES {
        let (br, bl, r, l) = (base_remote.get(f), base_local.get(f), remote.get(f), local.get(f));
        if r == br || r == l {
            continue; // Kontakt unverändert oder schon gleich
        }
        if l == bl {
            m.take.push((f, r.to_string()));
        } else {
            m.conflicts.push(f);
        }
    }
    m
}

/// FNV-1a 64 Bit: stabiler Hash für den schnellen Vergleich (unabhängig von der Rust-Version)
fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn input_of(p: &Person, f: &Fields) -> PersonInput {
    PersonInput {
        id: Some(p.id.clone()),
        name: f.name.clone(),
        email: f.email.clone(),
        phone: f.phone.clone(),
        street: f.street.clone(),
        house_no: f.house_no.clone(),
        zip: f.zip.clone(),
        city: f.city.clone(),
        billing_differs: p.billing_differs != 0,
        billing_name: p.billing_name.clone(),
        billing_street: p.billing_street.clone(),
        billing_house_no: p.billing_house_no.clone(),
        billing_zip: p.billing_zip.clone(),
        billing_city: p.billing_city.clone(),
        note: p.note.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn save_link(store: &Store, connector: &str, c: &ExtContact, person_id: &str, remote: &Fields, local: &Fields, state: &str, conflict: &[&str]) -> Result<()> {
    let r = remote.to_json();
    let l = local.to_json();
    store
        .upsert_link(&LinkInput {
            connector: connector.into(),
            entity: ENTITY.into(),
            external_id: c.external_id.clone(),
            local_id: person_id.into(),
            etag: c.etag.clone(),
            remote_updated_at: None,
            synced_hash: hash(&r),
            local_hash: hash(&l),
            remote_snapshot: r,
            local_snapshot: l,
            state: state.into(),
            conflict: conflict.join(","),
        })
        .await?;
    Ok(())
}

async fn person(store: &Store, id: &str) -> Result<Person> {
    store.get_person(id).await?.ok_or_else(|| AppError::NotFound("Kunde".into()))
}

/// Bestehenden Kunden mit einem Kontakt verknüpfen. Leere Felder des Kunden werden aus dem Kontakt gefüllt,
/// vorhandene Werte bleiben. Danach gilt der jetzige Stand beider Seiten als gemeinsamer Stand.
pub async fn link(store: &Store, connector: &str, c: &ExtContact, person_id: &str) -> Result<Vec<&'static str>> {
    if let Some(l) = store.link_by_external(connector, ENTITY, &c.external_id).await? {
        if l.local_id != person_id {
            let other = person(store, &l.local_id).await.map(|p| p.name).unwrap_or_default();
            return Err(AppError::Invalid(format!("Dieser Kontakt ist schon mit dem Kunden „{other}“ verknüpft.")));
        }
    }
    let p = person(store, person_id).await?;
    let remote = from_contact(c);
    let mut local = Fields::of_person(&p);
    let mut filled = Vec::new();
    for f in FIELD_NAMES {
        if local.get(f).trim().is_empty() && !remote.get(f).is_empty() {
            local.set(f, remote.get(f).to_string());
            filled.push(f);
        }
    }
    if !filled.is_empty() {
        store.save_person(input_of(&p, &local)).await?;
    }
    save_link(store, connector, c, person_id, &remote, &local, "ok", &[]).await?;
    Ok(filled)
}

/// Neuen Kunden aus einem Kontakt anlegen und verknüpfen. Ist der Kontakt schon verknüpft, gilt dieser Kunde;
/// gibt es einen Kunden gleichen Namens, wird er verknüpft statt doppelt angelegt.
pub async fn import(store: &Store, connector: &str, c: &ExtContact) -> Result<(String, String)> {
    if let Some(l) = store.link_by_external(connector, ENTITY, &c.external_id).await? {
        if store.get_person(&l.local_id).await?.is_some() {
            return Ok((l.local_id, "Dieser Kontakt ist schon als Kunde übernommen.".into()));
        }
        store.delete_link(&l.id).await?; // Kunde gelöscht: alte Verknüpfung weg
    }
    let f = from_contact(c);
    if f.name.trim().is_empty() {
        return Err(AppError::Invalid("Der Kontakt hat keinen Namen.".into()));
    }
    if let Some(twin) = store.list_people().await?.into_iter().find(|p| p.name.trim().eq_ignore_ascii_case(f.name.trim())) {
        link(store, connector, c, &twin.id).await?;
        return Ok((twin.id, format!("Bestehender Kunde „{}“ mit dem Kontakt verknüpft.", twin.name)));
    }
    let id = store
        .save_person(PersonInput {
            id: None,
            name: f.name.clone(),
            email: f.email.clone(),
            phone: f.phone.clone(),
            street: f.street.clone(),
            house_no: f.house_no.clone(),
            zip: f.zip.clone(),
            city: f.city.clone(),
            billing_differs: false,
            billing_name: String::new(),
            billing_street: String::new(),
            billing_house_no: String::new(),
            billing_zip: String::new(),
            billing_city: String::new(),
            note: String::new(),
        })
        .await?;
    let p = person(store, &id).await?;
    save_link(store, connector, c, &id, &f, &Fields::of_person(&p), "ok", &[]).await?;
    Ok((id, format!("Kunde „{}“ aus dem Kontakt angelegt.", f.name)))
}

/// Ergebnis eines Abgleichs
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct SyncReport {
    pub updated: Vec<String>,
    pub conflicts: Vec<String>,
    pub gone: Vec<String>,
}

impl SyncReport {
    pub fn messages(&self, source: &str) -> Vec<String> {
        let mut m = Vec::new();
        for u in &self.updated {
            m.push(format!("{u}: aus {source} aktualisiert"));
        }
        for c in &self.conflicts {
            m.push(format!("{c}: Kontakt und Kunde wurden beide geändert – bitte beim Kunden entscheiden"));
        }
        for g in &self.gone {
            m.push(format!("{g}: den verknüpften Kontakt gibt es in {source} nicht mehr"));
        }
        m
    }
}

/// Verknüpfte Kunden einer Quelle abgleichen. `contacts` ist die vollständige Liste der Quelle.
pub async fn sync(store: &Store, connector: &str, contacts: &[ExtContact]) -> Result<SyncReport> {
    let mut rep = SyncReport::default();
    for l in store.links_for_connector(connector, ENTITY).await? {
        let Some(p) = store.get_person(&l.local_id).await? else {
            store.delete_link(&l.id).await?; // Kunde gelöscht
            continue;
        };
        let Some(c) = contacts.iter().find(|c| c.external_id == l.external_id) else {
            if l.state != "geloescht" {
                set_state(store, &l, "geloescht", "").await?;
                rep.gone.push(p.name.clone());
            }
            continue;
        };
        let remote = from_contact(c);
        let local = Fields::of_person(&p);
        let m = merge(&Fields::from_json(&l.remote_snapshot), &Fields::from_json(&l.local_snapshot), &remote, &local);
        let mut next_local = local.clone();
        for (f, v) in &m.take {
            next_local.set(f, v.clone());
        }
        if !m.take.is_empty() {
            store.save_person(input_of(&p, &next_local)).await?;
            rep.updated.push(format!("{} ({})", next_local.name, m.take.iter().map(|(f, _)| field_label(f)).collect::<Vec<_>>().join(", ")));
        }
        // Der gemeinsame Stand eines Feldes rückt nur vor, wenn beide Seiten jetzt übereinstimmen.
        // Sonst bleibt er, damit eine Änderung nur in der App (oder ein Konflikt) erkennbar bleibt.
        let (base_remote, base_local) = advance_base(&l, &remote, &next_local);
        let state = if m.conflicts.is_empty() { "ok" } else { "konflikt" };
        if !m.conflicts.is_empty() && l.state != "konflikt" {
            rep.conflicts.push(p.name.clone());
        }
        save_link(store, connector, c, &p.id, &base_remote, &base_local, state, &m.conflicts).await?;
    }
    Ok(rep)
}

async fn set_state(store: &Store, l: &ExternalLink, state: &str, conflict: &str) -> Result<()> {
    store
        .upsert_link(&LinkInput {
            connector: l.connector.clone(),
            entity: l.entity.clone(),
            external_id: l.external_id.clone(),
            local_id: l.local_id.clone(),
            etag: l.etag.clone(),
            remote_updated_at: l.remote_updated_at.clone(),
            synced_hash: l.synced_hash.clone(),
            local_hash: l.local_hash.clone(),
            remote_snapshot: l.remote_snapshot.clone(),
            local_snapshot: l.local_snapshot.clone(),
            state: state.into(),
            conflict: conflict.into(),
        })
        .await?;
    Ok(())
}

fn advance_base(l: &ExternalLink, remote: &Fields, local: &Fields) -> (Fields, Fields) {
    let mut br = Fields::from_json(&l.remote_snapshot);
    let mut bl = Fields::from_json(&l.local_snapshot);
    for f in FIELD_NAMES {
        if remote.get(f) == local.get(f) || (remote.get(f) == br.get(f) && local.get(f) == bl.get(f)) {
            br.set(f, remote.get(f).to_string());
            bl.set(f, local.get(f).to_string());
        }
    }
    (br, bl)
}

/// Konflikt auflösen: `take_remote` übernimmt die Werte des Kontakts für die Konfliktfelder, sonst bleibt die App.
pub async fn resolve(store: &Store, connector: &str, c: &ExtContact, person_id: &str, take_remote: bool) -> Result<()> {
    let l = store
        .link_by_external(connector, ENTITY, &c.external_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Verknüpfung".into()))?;
    let p = person(store, person_id).await?;
    let remote = from_contact(c);
    let mut local = Fields::of_person(&p);
    if take_remote {
        for f in l.conflict.split(',').filter(|f| !f.is_empty()) {
            if let Some(name) = FIELD_NAMES.iter().find(|n| **n == f) {
                local.set(name, remote.get(name).to_string());
            }
        }
        store.save_person(input_of(&p, &local)).await?;
    }
    save_link(store, connector, c, person_id, &remote, &local, "ok", &[]).await
}

// ---------- Quellen ----------

/// Alle eingeschalteten Kontaktquellen mit ihren Kontakten
async fn sources(reg: &Registry) -> Vec<(&'static str, &'static str, Result<Vec<ExtContact>>)> {
    let mut out = Vec::new();
    for e in reg.entries() {
        let d = e.connector.descriptor();
        let Some(src) = e.connector.contacts() else { continue };
        if !reg.cap_on(d.id, Cap::ContactsRead).await || !e.connector.status(&e.ctx).await.connected {
            continue;
        }
        out.push((d.id, d.name, src.list_all(&e.ctx).await));
    }
    out
}

async fn contact(reg: &Registry, connector: &str, external_id: &str) -> Result<ExtContact> {
    let e = reg.get(connector)?;
    let src = e.connector.contacts().ok_or_else(|| AppError::Invalid(format!("„{connector}“ ist keine Kontaktquelle.")))?;
    src.list_all(&e.ctx)
        .await?
        .into_iter()
        .find(|c| c.external_id == external_id)
        .ok_or_else(|| AppError::NotFound("Kontakt".into()))
}

/// Alle verknüpften Kunden aller Quellen abgleichen (Takt und „Jetzt abgleichen“).
pub async fn sync_all(store: &Store, reg: &Registry) -> Result<Vec<String>> {
    let mut messages = Vec::new();
    for (id, name, list) in sources(reg).await {
        let r = sync(store, id, &list?).await?;
        messages.extend(r.messages(name));
    }
    Ok(messages)
}

// ---------- Befehle ----------

use crate::commands::AppState;
use tauri::State;

/// Treffer der Kontaktsuche (nur für die Auswahl in der Oberfläche)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactHit {
    pub connector: &'static str,
    pub source: &'static str,
    pub external_id: String,
    pub name: String,
    pub organization: String,
    pub email: String,
    pub phone: String,
    pub city: String,
    /// schon mit diesem Kunden verknüpft
    pub person_id: Option<String>,
}

fn matches(c: &ExtContact, q: &str) -> bool {
    if q.is_empty() {
        return true;
    }
    let digits: String = q.chars().filter(|x| x.is_ascii_digit()).collect();
    let hay = format!("{} {} {} {}", c.given_name, c.family_name, c.display_name, c.organization).to_lowercase();
    q.split_whitespace().all(|w| hay.contains(w))
        || c.emails.iter().any(|e| e.value.to_lowercase().contains(q))
        || (digits.len() >= 4 && c.phones.iter().any(|p| p.value.chars().filter(|x| x.is_ascii_digit()).collect::<String>().contains(&digits)))
}

/// Kontakte aller eingeschalteten Quellen durchsuchen (höchstens 40 Treffer).
#[tauri::command]
pub async fn contacts_search(state: State<'_, AppState>, registry: State<'_, Arc<Registry>>, query: String) -> Result<Vec<ContactHit>> {
    let q = query.trim().to_lowercase();
    let mut out = Vec::new();
    for (id, name, list) in sources(&registry).await {
        let ctx = state.ctx.read().await;
        let linked: std::collections::HashMap<String, String> =
            ctx.store.links_for_connector(id, ENTITY).await?.into_iter().map(|l| (l.external_id, l.local_id)).collect();
        for c in list?.into_iter().filter(|c| matches(c, &q)) {
            let f = from_contact(&c);
            if f.name.is_empty() {
                continue;
            }
            out.push(ContactHit {
                connector: id,
                source: name,
                person_id: linked.get(&c.external_id).cloned(),
                name: f.name,
                organization: c.organization.clone(),
                email: f.email,
                phone: f.phone,
                city: f.city,
                external_id: c.external_id,
            });
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out.truncate(40);
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub person_id: String,
    pub message: String,
}

/// Kunden aus einem Kontakt anlegen (oder den schon verknüpften bzw. gleichnamigen Kunden nehmen).
#[tauri::command]
pub async fn person_from_contact(state: State<'_, AppState>, registry: State<'_, Arc<Registry>>, connector: String, external_id: String) -> Result<ImportResult> {
    let c = contact(&registry, &connector, &external_id).await?;
    let ctx = state.ctx.read().await;
    let (person_id, message) = import(&ctx.store, registry.get(&connector)?.connector.descriptor().id, &c).await?;
    Ok(ImportResult { person_id, message })
}

/// Bestehenden Kunden mit einem Kontakt verknüpfen; leere Felder werden gefüllt.
#[tauri::command]
pub async fn person_link_contact(
    state: State<'_, AppState>,
    registry: State<'_, Arc<Registry>>,
    person_id: String,
    connector: String,
    external_id: String,
) -> Result<String> {
    let c = contact(&registry, &connector, &external_id).await?;
    let ctx = state.ctx.read().await;
    let id = registry.get(&connector)?.connector.descriptor().id;
    // höchstens eine Verknüpfung je Kunde und Quelle
    for l in ctx.store.links_for_local(ENTITY, &person_id).await?.into_iter().filter(|l| l.connector == id) {
        ctx.store.delete_link(&l.id).await?;
    }
    let filled = link(&ctx.store, id, &c, &person_id).await?;
    Ok(if filled.is_empty() {
        "Mit dem Kontakt verknüpft.".into()
    } else {
        format!("Mit dem Kontakt verknüpft, ergänzt: {}.", filled.iter().map(|f| field_label(f)).collect::<Vec<_>>().join(", "))
    })
}

/// Verknüpfung(en) eines Kunden lösen. Der Kunde bleibt unverändert.
#[tauri::command]
pub async fn person_unlink_contact(state: State<'_, AppState>, person_id: String) -> Result<()> {
    let ctx = state.ctx.read().await;
    for l in ctx.store.links_for_local(ENTITY, &person_id).await? {
        ctx.store.delete_link(&l.id).await?;
    }
    Ok(())
}

/// Verknüpfung eines Kunden für die Anzeige
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonLink {
    pub connector: String,
    pub source: String,
    pub external_id: String,
    /// ok | konflikt | geloescht
    pub state: String,
    pub last_synced_at: Option<String>,
    /// Konfliktfelder mit Wert im Kontakt und beim Kunden
    pub conflicts: Vec<ConflictField>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictField {
    pub field: String,
    pub label: String,
    pub contact: String,
    pub customer: String,
}

#[tauri::command]
pub async fn person_contact(state: State<'_, AppState>, registry: State<'_, Arc<Registry>>, person_id: String) -> Result<Option<PersonLink>> {
    let (l, p) = {
        let ctx = state.ctx.read().await;
        let Some(l) = ctx.store.links_for_local(ENTITY, &person_id).await?.into_iter().next() else { return Ok(None) };
        (l, person(&ctx.store, &person_id).await?)
    };
    let source = registry.get(&l.connector).map(|e| e.connector.descriptor().name.to_string()).unwrap_or_else(|_| l.connector.clone());
    let mut conflicts = Vec::new();
    if l.state == "konflikt" {
        let remote = match contact(&registry, &l.connector, &l.external_id).await {
            Ok(c) => from_contact(&c),
            Err(_) => Fields::from_json(&l.remote_snapshot),
        };
        let local = Fields::of_person(&p);
        for f in l.conflict.split(',').filter(|f| !f.is_empty()) {
            conflicts.push(ConflictField {
                field: f.to_string(),
                label: field_label(f).into(),
                contact: remote.get(f).to_string(),
                customer: local.get(f).to_string(),
            });
        }
    }
    Ok(Some(PersonLink { connector: l.connector, source, external_id: l.external_id, state: l.state, last_synced_at: l.last_synced_at, conflicts }))
}

#[tauri::command]
pub async fn person_contact_resolve(state: State<'_, AppState>, registry: State<'_, Arc<Registry>>, person_id: String, take_contact: bool) -> Result<()> {
    let l = {
        let ctx = state.ctx.read().await;
        ctx.store.links_for_local(ENTITY, &person_id).await?.into_iter().next().ok_or_else(|| AppError::NotFound("Verknüpfung".into()))?
    };
    let c = contact(&registry, &l.connector, &l.external_id).await?;
    let ctx = state.ctx.read().await;
    resolve(&ctx.store, &l.connector, &c, &person_id, take_contact).await
}

/// Verknüpfte Kunden jetzt abgleichen.
#[tauri::command]
pub async fn contacts_sync_now(state: State<'_, AppState>, registry: State<'_, Arc<Registry>>) -> Result<Vec<String>> {
    let ctx = state.ctx.read().await;
    sync_all(&ctx.store, &registry).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(id: &str, given: &str, phone: &str, street: &str) -> ExtContact {
        ExtContact {
            external_id: id.into(),
            given_name: given.into(),
            family_name: "Muster".into(),
            phones: vec![Typed { kind: "privat".into(), value: "0711 1".into() }, Typed { kind: "mobil".into(), value: phone.into() }],
            addresses: vec![Typed { kind: "".into(), value: PostalAddress { street: street.into(), zip: "70173".into(), city: "Stuttgart".into(), country: String::new() } }],
            ..Default::default()
        }
    }

    #[test]
    fn strasse_und_hausnummer() {
        assert_eq!(split_street("Musterstraße 12a"), ("Musterstraße".into(), "12a".into()));
        assert_eq!(split_street("Am Ring 3 - 5"), ("Am Ring".into(), "3-5".into()));
        assert_eq!(split_street("Hauptstr. 7/2"), ("Hauptstr.".into(), "7/2".into()));
        assert_eq!(split_street("12 Main Street"), ("Main Street".into(), "12".into()));
        assert_eq!(split_street("Postfach"), ("Postfach".into(), "".into()));
        assert_eq!(split_street("Weg 4\nHinterhaus"), ("Weg, Hinterhaus".into(), "4".into()));
        assert_eq!(split_street("Straße des 17. Juni 100"), ("Straße des 17. Juni".into(), "100".into()));
    }

    #[test]
    fn kontakt_zu_kunde() {
        let f = from_contact(&contact("1", "Anna", "+49 170 1", "Musterstraße 12a"));
        assert_eq!(f.name, "Anna Muster");
        assert_eq!(f.phone, "+49 170 1", "Mobil vor Festnetz");
        assert_eq!((f.street.as_str(), f.house_no.as_str(), f.zip.as_str()), ("Musterstraße", "12a", "70173"));
        let firma = from_contact(&ExtContact { organization: "Werkstatt GmbH".into(), ..Default::default() });
        assert_eq!(firma.name, "Werkstatt GmbH");
    }

    #[test]
    fn vergleich_je_feld() {
        let base = Fields { name: "A".into(), phone: "1".into(), city: "X".into(), ..Default::default() };
        // nur Kontakt geändert → übernehmen
        let m = merge(&base, &base, &Fields { phone: "2".into(), ..base.clone() }, &base);
        assert_eq!(m.take, vec![("phone", "2".to_string())]);
        // nur Kunde geändert → bleibt
        let m = merge(&base, &base, &base, &Fields { city: "Y".into(), ..base.clone() });
        assert_eq!(m, Merge::default());
        // beide verschieden → Konflikt; beide gleich → nichts
        let m = merge(&base, &base, &Fields { city: "Y".into(), ..base.clone() }, &Fields { city: "Z".into(), ..base.clone() });
        assert_eq!(m.conflicts, vec!["city"]);
        let m = merge(&base, &base, &Fields { city: "Y".into(), ..base.clone() }, &Fields { city: "Y".into(), ..base.clone() });
        assert_eq!(m, Merge::default());
    }

    async fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::connect(&Store::sqlite_url(&dir.path().join("t.db"))).await.unwrap();
        (s, dir)
    }

    #[tokio::test]
    async fn uebernehmen_verknuepfen_abgleichen() {
        let (s, _d) = store().await;
        let c = contact("k1", "Anna", "+49 170 1", "Musterstraße 12a");
        // übernehmen legt einen Kunden an, ein zweites Mal nicht
        let (id, msg) = import(&s, "apple-contacts", &c).await.unwrap();
        assert!(msg.contains("angelegt"));
        let p = s.get_person(&id).await.unwrap().unwrap();
        assert_eq!((p.name.as_str(), p.street.as_str(), p.house_no.as_str()), ("Anna Muster", "Musterstraße", "12a"));
        assert_eq!(import(&s, "apple-contacts", &c).await.unwrap().0, id);

        // Kontakt geändert (Umzug) → übernommen
        let moved = contact("k1", "Anna", "+49 170 1", "Neuer Weg 5");
        let r = sync(&s, "apple-contacts", std::slice::from_ref(&moved)).await.unwrap();
        assert_eq!(r.updated.len(), 1);
        let p = s.get_person(&id).await.unwrap().unwrap();
        assert_eq!((p.street.as_str(), p.house_no.as_str()), ("Neuer Weg", "5"));

        // Kunde in der App geändert, Kontakt nicht → bleibt
        let mut inp = input_of(&p, &Fields { phone: "0171 9".into(), ..Fields::of_person(&p) });
        inp.note = "Notiz bleibt".into();
        s.save_person(inp).await.unwrap();
        assert_eq!(sync(&s, "apple-contacts", std::slice::from_ref(&moved)).await.unwrap(), SyncReport::default());
        assert_eq!(s.get_person(&id).await.unwrap().unwrap().phone, "0171 9");

        // beide ändern das Telefon → Konflikt, einmal gemeldet
        let both = contact("k1", "Anna", "+49 160 7", "Neuer Weg 5");
        let r = sync(&s, "apple-contacts", std::slice::from_ref(&both)).await.unwrap();
        assert_eq!(r.conflicts, vec!["Anna Muster".to_string()]);
        assert!(sync(&s, "apple-contacts", std::slice::from_ref(&both)).await.unwrap().conflicts.is_empty(), "nur einmal melden");
        let l = s.links_for_local(ENTITY, &id).await.unwrap().remove(0);
        assert_eq!((l.state.as_str(), l.conflict.as_str()), ("konflikt", "phone"));
        // Kontakt übernehmen löst den Konflikt
        resolve(&s, "apple-contacts", &both, &id, true).await.unwrap();
        let p = s.get_person(&id).await.unwrap().unwrap();
        assert_eq!((p.phone.as_str(), p.note.as_str()), ("+49 160 7", "Notiz bleibt"));
        assert_eq!(s.links_for_local(ENTITY, &id).await.unwrap()[0].state, "ok");

        // Kontakt gelöscht → einmal gemeldet, Kunde bleibt
        let r = sync(&s, "apple-contacts", &[]).await.unwrap();
        assert_eq!(r.gone.len(), 1);
        assert!(sync(&s, "apple-contacts", &[]).await.unwrap().gone.is_empty());
        assert!(s.get_person(&id).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn verknuepfen_fuellt_nur_leere_felder() {
        let (s, _d) = store().await;
        let id = s
            .save_person(PersonInput { name: "Anna Muster".into(), phone: "0711 5".into(), ..Default::default() })
            .await
            .unwrap();
        let c = contact("k2", "Anna", "+49 170 1", "Musterstraße 12a");
        let filled = link(&s, "apple-contacts", &c, &id).await.unwrap();
        assert!(filled.contains(&"street") && !filled.contains(&"phone"));
        let p = s.get_person(&id).await.unwrap().unwrap();
        assert_eq!((p.phone.as_str(), p.city.as_str()), ("0711 5", "Stuttgart"));
        // gleicher Name beim Übernehmen → verknüpft statt doppelt
        let other = contact("k3", "Anna", "+49 1", "");
        let (id2, msg) = import(&s, "apple-contacts", &other).await.unwrap();
        assert_eq!(id2, id);
        assert!(msg.contains("verknüpft"));
        // derselbe Kontakt an einem anderen Kunden: abgelehnt
        let id3 = s.save_person(PersonInput { name: "Bert".into(), ..Default::default() }).await.unwrap();
        assert!(link(&s, "apple-contacts", &c, &id3).await.is_err());
    }
}
