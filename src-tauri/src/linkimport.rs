//! Links von Modell-Plattformen auswerten: Titel, Beschreibung, Vorschaubild, Dateien.
//!
//! - Printables: öffentliche GraphQL-API, Dateien ohne Anmeldung ladbar
//! - MakerWorld: öffentliche API für Infos und Druckprofile; Dateien nur mit Anmeldung
//! - sonst: OpenGraph-Angaben der Seite, soweit die Seite sie herausgibt

use crate::error::{AppError, Result};
use crate::threemf::html_to_text;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::LazyLock;
use std::time::Duration;

const UA: &str = concat!("Printmanager/", env!("CARGO_PKG_VERSION"), " (Desktop-App)");

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub size: Option<i64>,
    /// stl | gcode | other (Printables-Dateityp für den Download)
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub title: String,
    pub print_seconds: Option<i64>,
    pub weight_grams: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LinkPreview {
    /// printables | makerworld | thingiverse | web
    pub source: String,
    pub source_label: String,
    pub url: String,
    pub remote_id: Option<String>,
    pub title: String,
    pub description: String,
    pub image_url: Option<String>,
    pub designer: String,
    pub license: String,
    pub files: Vec<RemoteFile>,
    pub profiles: Vec<Profile>,
    /// Hinweis für die Oberfläche, z.B. warum keine Dateien geladen werden können
    pub note: Option<String>,
}

static RE_PRINTABLES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"printables\.com/(?:[a-z]{2}/)?model/(\d+)").unwrap());
static RE_MAKERWORLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"makerworld\.com(?:\.cn)?/(?:[a-z]{2}(?:-[a-z]{2})?/)?models/(\d+)").unwrap());
static RE_THINGIVERSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"thingiverse\.com/thing:(\d+)").unwrap());
static RE_OG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<meta[^>]+(?:property|name)=["'](og:title|og:description|og:image|twitter:image|description)["'][^>]*>"#).unwrap()
});
static RE_CONTENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)content=["']([^"']*)["']"#).unwrap());
static RE_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());

/// Welche Plattform steckt hinter dem Link?
pub fn detect(url: &str) -> (&'static str, Option<String>) {
    if let Some(c) = RE_PRINTABLES.captures(url) {
        return ("printables", Some(c[1].to_string()));
    }
    if let Some(c) = RE_MAKERWORLD.captures(url) {
        return ("makerworld", Some(c[1].to_string()));
    }
    if let Some(c) = RE_THINGIVERSE.captures(url) {
        return ("thingiverse", Some(c[1].to_string()));
    }
    ("web", None)
}

fn label(source: &str) -> &'static str {
    match source {
        "printables" => "Printables",
        "makerworld" => "MakerWorld",
        "thingiverse" => "Thingiverse",
        _ => "Web",
    }
}

pub fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(UA)
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| AppError::Invalid(format!("HTTP-Client: {e}")))
}

fn net(e: reqwest::Error) -> AppError {
    AppError::Invalid(format!("Abruf fehlgeschlagen: {e}"))
}

pub async fn preview(url: &str) -> Result<LinkPreview> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Invalid("Bitte einen Link mit http:// oder https:// angeben.".into()));
    }
    let (source, id) = detect(url);
    let http = client()?;
    let mut p = match (source, id.as_deref()) {
        ("printables", Some(id)) => printables(&http, id).await?,
        ("makerworld", Some(id)) => makerworld(&http, url, id).await?,
        _ => web(&http, url).await,
    };
    p.source = source.into();
    p.source_label = label(source).into();
    p.url = url.into();
    p.remote_id = id;
    if source == "thingiverse" && p.note.is_none() {
        p.note = Some("Thingiverse gibt Dateien nur über die Webseite heraus. Bitte dort herunterladen und hineinziehen.".into());
    }
    if p.title.is_empty() {
        p.title = url.split("://").nth(1).unwrap_or(url).trim_end_matches('/').to_string();
    }
    Ok(p)
}

const PRINTABLES_GQL: &str = "https://api.printables.com/graphql/";

async fn printables(http: &reqwest::Client, id: &str) -> Result<LinkPreview> {
    let q = "query($id:ID!){print(id:$id){id name summary description user{publicUsername} license{abbreviation name}
             images{filePath} stls{id name fileSize} gcodes{id name fileSize} otherFiles{id name fileSize}}}";
    let v: Value = http
        .post(PRINTABLES_GQL)
        .json(&json!({"query": q, "variables": {"id": id}}))
        .send()
        .await
        .map_err(net)?
        .json()
        .await
        .map_err(net)?;
    let pr = &v["data"]["print"];
    if pr.is_null() {
        return Err(AppError::Invalid("Modell auf Printables nicht gefunden.".into()));
    }
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let mut files = vec![];
    for (key, kind) in [("stls", "stl"), ("otherFiles", "other"), ("gcodes", "gcode")] {
        for f in pr[key].as_array().into_iter().flatten() {
            files.push(RemoteFile { id: s(&f["id"]), name: s(&f["name"]), size: f["fileSize"].as_i64(), kind: kind.into() });
        }
    }
    let summary = s(&pr["summary"]);
    let desc = html_to_text(&s(&pr["description"]));
    Ok(LinkPreview {
        title: s(&pr["name"]),
        description: if desc.is_empty() { summary } else { desc },
        image_url: pr["images"][0]["filePath"].as_str().map(|p| format!("https://media.printables.com/{p}")),
        designer: s(&pr["user"]["publicUsername"]),
        license: [s(&pr["license"]["abbreviation"]), s(&pr["license"]["name"])].into_iter().find(|x| !x.is_empty()).unwrap_or_default(),
        files,
        ..Default::default()
    })
}

async fn makerworld(http: &reqwest::Client, url: &str, id: &str) -> Result<LinkPreview> {
    let api = if url.contains("makerworld.com.cn") { "https://makerworld.com.cn" } else { "https://makerworld.com" };
    let v: Value = http
        .get(format!("{api}/api/v1/design-service/design/{id}"))
        .send()
        .await
        .map_err(net)?
        .json()
        .await
        .map_err(net)?;
    if v["id"].as_i64().unwrap_or(0) == 0 {
        return Err(AppError::Invalid("Modell auf MakerWorld nicht gefunden.".into()));
    }
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let profiles = v["instances"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|i| Profile {
            title: s(&i["title"]),
            print_seconds: i["prediction"].as_i64().filter(|x| *x > 0),
            weight_grams: i["weight"].as_f64().filter(|x| *x > 0.0),
        })
        .collect();
    Ok(LinkPreview {
        title: s(&v["title"]),
        description: html_to_text(&s(&v["summary"])),
        image_url: v["coverUrl"].as_str().filter(|u| !u.is_empty()).map(String::from),
        designer: s(&v["designCreator"]["name"]),
        license: s(&v["license"]),
        profiles,
        note: Some(
            "MakerWorld gibt Dateien nur mit Anmeldung heraus. Lade die 3MF dort (oder in Bambu Studio) herunter \
             und zieh sie in den Auftrag."
                .into(),
        ),
        ..Default::default()
    })
}

/// Allgemeine Seite: OpenGraph-Angaben. Fehler sind hier kein Abbruch, dann bleibt es beim Link.
async fn web(http: &reqwest::Client, url: &str) -> LinkPreview {
    let mut p = LinkPreview::default();
    let html = match http.get(url).send().await {
        Ok(r) if r.status().is_success() => r.text().await.unwrap_or_default(),
        Ok(r) => {
            p.note = Some(format!("Die Seite hat keine Vorschau herausgegeben (HTTP {}). Der Link wird trotzdem gespeichert.", r.status().as_u16()));
            return p;
        }
        Err(_) => {
            p.note = Some("Die Seite war nicht erreichbar. Der Link wird trotzdem gespeichert.".into());
            return p;
        }
    };
    let head = &html[..floor(&html, 200_000)];
    for m in RE_OG.captures_iter(head) {
        let content = RE_CONTENT
            .captures(m.get(0).unwrap().as_str())
            .map(|c| crate::threemf::decode_entities(&c[1]))
            .unwrap_or_default();
        match m[1].to_lowercase().as_str() {
            "og:title" if p.title.is_empty() => p.title = content,
            "og:description" | "description" if p.description.is_empty() => p.description = content,
            "og:image" | "twitter:image" if p.image_url.is_none() && content.starts_with("http") => p.image_url = Some(content),
            _ => {}
        }
    }
    if p.title.is_empty() {
        if let Some(t) = RE_TITLE.captures(head) {
            p.title = crate::threemf::decode_entities(t[1].trim());
        }
    }
    if p.title.contains("Just a moment") {
        p.title.clear();
        p.note = Some("Die Seite blockiert automatische Abrufe. Der Link wird trotzdem gespeichert.".into());
    }
    p
}

fn floor(s: &str, n: usize) -> usize {
    let mut e = s.len().min(n);
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    e
}

/// Datei herunterladen (Bytes). Bei Printables wird zuerst ein Download-Link angefordert.
pub async fn download_file(p: &LinkPreview, f: &RemoteFile) -> Result<Vec<u8>> {
    if p.source != "printables" {
        return Err(AppError::Invalid(format!("{} erlaubt keinen direkten Download.", p.source_label)));
    }
    let http = client()?;
    let q = "mutation($id:ID!,$printId:ID!,$fileType:DownloadFileTypeEnum!,$source:DownloadSourceEnum!){
             getDownloadLink(id:$id,printId:$printId,fileType:$fileType,source:$source){ok output{link}}}";
    let v: Value = http
        .post(PRINTABLES_GQL)
        .json(&json!({"query": q, "variables": {
            "id": f.id, "printId": p.remote_id, "fileType": f.kind, "source": "model_detail"}}))
        .send()
        .await
        .map_err(net)?
        .json()
        .await
        .map_err(net)?;
    let link = v["data"]["getDownloadLink"]["output"]["link"]
        .as_str()
        .ok_or_else(|| AppError::Invalid(format!("Kein Download-Link für {}", f.name)))?;
    get_bytes(&http, link).await
}

pub async fn download_image(url: &str) -> Result<Vec<u8>> {
    get_bytes(&client()?, url).await
}

async fn get_bytes(http: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let r = http.get(url).send().await.map_err(net)?;
    if !r.status().is_success() {
        return Err(AppError::Invalid(format!("Download fehlgeschlagen (HTTP {})", r.status().as_u16())));
    }
    Ok(r.bytes().await.map_err(net)?.to_vec())
}

/// Dateiendung für ein heruntergeladenes Bild anhand der ersten Bytes
pub fn image_ext(b: &[u8]) -> &'static str {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "webp"
    } else if b.starts_with(b"GIF8") {
        "gif"
    } else {
        "jpg"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plattform_erkennen() {
        assert_eq!(detect("https://www.printables.com/model/3161-3d-benchy").0, "printables");
        assert_eq!(detect("https://www.printables.com/de/model/3161-3d-benchy").1.as_deref(), Some("3161"));
        assert_eq!(detect("https://makerworld.com/en/models/1000000-lens-cap#profileId-1").1.as_deref(), Some("1000000"));
        assert_eq!(detect("https://makerworld.com/de/models/42").0, "makerworld");
        assert_eq!(detect("https://www.thingiverse.com/thing:763622").1.as_deref(), Some("763622"));
        assert_eq!(detect("https://example.com/x").0, "web");
        assert_eq!(image_ext(&[0x89, b'P', b'N', b'G', 0]), "png");
    }

    /// Echte Abrufe nur mit `PM_NET_TESTS=1`, damit `cargo test` offline läuft.
    #[tokio::test]
    async fn echte_abrufe() {
        if std::env::var("PM_NET_TESTS").is_err() {
            return;
        }
        let p = preview("https://www.printables.com/model/3161-3d-benchy").await.unwrap();
        assert_eq!(p.source, "printables");
        assert!(p.title.to_lowercase().contains("benchy"));
        assert!(p.image_url.is_some());
        let stl = p.files.iter().find(|f| f.name.ends_with(".stl")).unwrap();
        let bytes = download_file(&p, stl).await.unwrap();
        assert!(bytes.len() > 1_000_000);

        let m = preview("https://makerworld.com/en/models/1000000").await.unwrap();
        assert_eq!(m.source, "makerworld");
        assert!(!m.title.is_empty() && m.image_url.is_some() && !m.profiles.is_empty());
        assert!(download_image(m.image_url.as_deref().unwrap()).await.unwrap().len() > 1000);
    }
}
