//! Kontakte als Fähigkeit (`Cap::ContactsRead`, nur lesen): neutrales Kontaktmodell und eine Prüf-Statistik.
//! Die Statistik enthält nur Zahlen und Formate, nie Namen oder Werte (Stufe K0, docs/PLAN-Anbindungen.md).

use super::ConnCtx;
use crate::error::Result;
use async_trait::async_trait;
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// Ein Eintrag mit Art (privat, arbeit, mobil, sonstige …)
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Typed<T> {
    pub kind: String,
    pub value: T,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostalAddress {
    /// Straße, oft samt Hausnummer
    pub street: String,
    pub zip: String,
    pub city: String,
    /// ISO-Ländercode oder Ländername, wie die Quelle ihn liefert
    pub country: String,
}

/// Kontakt aus einer Quelle (Google, Apple, CardDAV), unabhängig vom Anbieter
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtContact {
    pub external_id: String,
    pub etag: String,
    pub display_name: String,
    pub given_name: String,
    pub family_name: String,
    pub organization: String,
    pub emails: Vec<Typed<String>>,
    pub phones: Vec<Typed<String>>,
    pub addresses: Vec<Typed<PostalAddress>>,
}

#[async_trait]
pub trait ContactSource: Send + Sync {
    /// Alle Kontakte der Quelle lesen.
    async fn list_all(&self, ctx: &ConnCtx) -> Result<Vec<ExtContact>>;
}

/// Ergebnis eines Prüflaufs: nur Zahlen und Formate
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeStats {
    pub count: usize,
    pub with_name: usize,
    /// nur Firma, kein Personenname
    pub only_organization: usize,
    pub with_email: usize,
    pub with_phone: usize,
    pub with_address: usize,
    /// Adressen, deren Straßenfeld eine Hausnummer enthält („Musterstraße 12a“)
    pub street_with_number: usize,
    pub street_without_number: usize,
    /// mehrzeilige Straßenfelder (z.B. Zusatzzeile)
    pub street_multiline: usize,
    /// Telefonformate: international (+49 …), national (0…), sonstige
    pub phone_formats: BTreeMap<String, usize>,
    /// Arten der Einträge (privat, arbeit, mobil …) über E-Mail, Telefon und Adresse
    pub kinds: BTreeMap<String, usize>,
    /// Länder der Adressen (Code oder Name, wie geliefert)
    pub countries: BTreeMap<String, usize>,
}

static RE_STREET_NR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\D+\s+\d+\s*[a-z]?(\s*[-/]\s*\d+\s*[a-z]?)?$").unwrap());
static RE_NR_FIRST: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+[a-zA-Z]?\s+\D+$").unwrap());

/// Hausnummer im Straßenfeld? Deutsch („Musterstraße 12a“) und englisch („12 Main Street“).
pub fn street_has_number(street: &str) -> bool {
    let s = street.lines().next().unwrap_or("").trim();
    RE_STREET_NR.is_match(s) || RE_NR_FIRST.is_match(s)
}

fn phone_format(p: &str) -> &'static str {
    let t = p.trim();
    if t.starts_with('+') || t.starts_with("00") {
        "international"
    } else if t.starts_with('0') {
        "national"
    } else {
        "sonstige"
    }
}

pub fn probe(list: &[ExtContact]) -> ProbeStats {
    let mut s = ProbeStats { count: list.len(), ..Default::default() };
    for c in list {
        let person = !(c.given_name.trim().is_empty() && c.family_name.trim().is_empty());
        if person || !c.display_name.trim().is_empty() {
            s.with_name += 1;
        }
        if !person && !c.organization.trim().is_empty() {
            s.only_organization += 1;
        }
        if !c.emails.is_empty() {
            s.with_email += 1;
        }
        if !c.phones.is_empty() {
            s.with_phone += 1;
        }
        if !c.addresses.is_empty() {
            s.with_address += 1;
        }
        for p in &c.phones {
            *s.phone_formats.entry(phone_format(&p.value).into()).or_default() += 1;
        }
        for k in c.emails.iter().map(|x| &x.kind).chain(c.phones.iter().map(|x| &x.kind)).chain(c.addresses.iter().map(|x| &x.kind)) {
            *s.kinds.entry(if k.is_empty() { "ohne".into() } else { k.clone() }).or_default() += 1;
        }
        for a in &c.addresses {
            if a.value.street.trim().is_empty() {
                continue;
            }
            if street_has_number(&a.value.street) {
                s.street_with_number += 1;
            } else {
                s.street_without_number += 1;
            }
            if a.value.street.trim().contains('\n') {
                s.street_multiline += 1;
            }
            let country = a.value.country.trim();
            *s.countries.entry(if country.is_empty() { "ohne".into() } else { country.to_uppercase() }).or_default() += 1;
        }
    }
    s
}

/// Art eines Eintrags vereinheitlichen (Apple `_$!<Home>!$_`, Google `home`, vCard `HOME`)
pub fn normalize_kind(label: &str) -> String {
    let l = label.trim().trim_start_matches("_$!<").trim_end_matches(">!$_").to_lowercase();
    match l.as_str() {
        "home" | "privat" | "zuhause" => "privat",
        "work" | "arbeit" | "geschäftlich" => "arbeit",
        "mobile" | "iphone" | "cell" | "mobil" => "mobil",
        "main" | "haupt" => "haupt",
        "" => "",
        _ => "sonstige",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(street: &str, phone: &str, kind: &str) -> ExtContact {
        ExtContact {
            given_name: "A".into(),
            phones: vec![Typed { kind: kind.into(), value: phone.into() }],
            addresses: vec![Typed { kind: kind.into(), value: PostalAddress { street: street.into(), country: "de".into(), ..Default::default() } }],
            ..Default::default()
        }
    }

    #[test]
    fn hausnummer_erkennen() {
        for s in ["Musterstraße 12", "Musterstraße 12a", "Am Ring 3 - 5", "Hauptstr. 7/2", "12 Main Street"] {
            assert!(street_has_number(s), "{s}");
        }
        for s in ["Musterstraße", "Postfach", ""] {
            assert!(!street_has_number(s), "{s}");
        }
    }

    #[test]
    fn statistik_ohne_werte() {
        let list = vec![
            c("Musterstraße 12a", "+49 170 1234567", "_$!<Home>!$_"),
            c("Ring", "0711 123", "work"),
            ExtContact { organization: "Firma".into(), ..Default::default() },
        ];
        let mut list = list;
        for x in &mut list {
            for p in &mut x.phones {
                p.kind = normalize_kind(&p.kind);
            }
            for a in &mut x.addresses {
                a.kind = normalize_kind(&a.kind);
            }
        }
        let s = probe(&list);
        assert_eq!((s.count, s.with_name, s.only_organization, s.with_phone, s.with_address), (3, 2, 1, 2, 2));
        assert_eq!((s.street_with_number, s.street_without_number), (1, 1));
        assert_eq!(s.phone_formats["international"], 1);
        assert_eq!(s.phone_formats["national"], 1);
        assert_eq!(s.kinds["privat"], 2);
        assert_eq!(s.countries["DE"], 2);
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("Muster") && !json.contains("170"), "keine Werte in der Statistik");
    }
}
