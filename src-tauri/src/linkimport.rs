//! Links von Modell-Plattformen auswerten: Titel, Beschreibung, Vorschaubild, Dateien.
//!
//! Jede Plattform ist ein `LinkResolver`; eine neue Plattform ist ein weiterer Resolver.
//! Alle Abrufe laufen über die gemeinsamen HTTP-Zugänge der Anbindungen (`connectors/http.rs`):
//! - Anbindung **Printables** (`api.printables.com`): öffentliche GraphQL-API, Dateien ohne Anmeldung ladbar
//! - Anbindung **Web**: Seitenvorschauen (OpenGraph), Bilder, Downloads von CDNs, MakerWorld-Infos;
//!   fremde Hosts, aber nie mit Token
//!
//! MakerWorld gibt Dateien nur mit Anmeldung heraus; das erledigt das Bambu-Konto (`bambu/makerworld.rs`).

use crate::connectors::http::HttpGateway;
use crate::connectors::registry::Registry;
use crate::connectors::{AuthKind, Cap, Descriptor, Kind, Platform};
use crate::error::{AppError, Result};
use crate::threemf::html_to_text;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};

const ALL: &[Platform] = &[Platform::MacOs, Platform::Windows, Platform::Linux];

/// Allgemeiner Web-Zugang: Seitenvorschauen, Bilder, Downloads. Nur GET, nie mit Token.
pub static WEB: Descriptor = Descriptor {
    id: "web",
    name: "Web",
    kind: Kind::Catalog,
    platforms: ALL,
    auth: AuthKind::None,
    caps: &[Cap::LinksRead],
    base_url: "",
    write_paths: &[],
    read_post_paths: &[],
    timeout_secs: 60,
    default_enabled: true,
    min_interval_ms: 250,
    intervals: &[],
    secrets: &[],
    heartbeat_secs: 0,
};

/// Printables: öffentliche GraphQL-API (Abfragen und Download-Links per POST)
pub static PRINTABLES: Descriptor = Descriptor {
    id: "printables",
    name: "Printables",
    kind: Kind::Catalog,
    platforms: ALL,
    auth: AuthKind::None,
    caps: &[Cap::LinksRead],
    base_url: "https://api.printables.com",
    write_paths: &[],
    read_post_paths: &[PRINTABLES_GQL],
    timeout_secs: 60,
    default_enabled: true,
    min_interval_ms: 500,
    intervals: &[],
    secrets: &[],
    heartbeat_secs: 0,
};

const PRINTABLES_GQL: &str = "/graphql/";

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

fn capture(re: &Regex, url: &str) -> Option<String> {
    re.captures(url).map(|c| c[1].to_string())
}

/// Eine Plattform für Modell-Links
#[async_trait]
pub trait LinkResolver: Send + Sync {
    /// Kennung für `LinkPreview::source`, z.B. "printables"
    fn source(&self) -> &'static str;
    fn label(&self) -> &'static str;
    /// Modell-ID im Link, falls der Link zu dieser Plattform gehört
    fn id_in(&self, url: &str) -> Option<String>;
    async fn preview(&self, url: &str, id: &str) -> Result<LinkPreview>;
    async fn download(&self, _p: &LinkPreview, _f: &RemoteFile) -> Result<Vec<u8>> {
        Err(AppError::Invalid(format!("{} erlaubt keinen direkten Download.", self.label())))
    }
}

/// Alle Resolver plus Web-Zugang für Vorschauen und Bilder
pub struct Links {
    resolvers: Vec<Box<dyn LinkResolver>>,
    web: Option<Arc<HttpGateway>>,
}

impl Links {
    /// Aus der Registry: nur eingeschaltete Anbindungen. Ohne Printables bleibt die Web-Vorschau.
    pub async fn new(reg: &Registry) -> Result<Self> {
        let gateway = |id: &str| reg.get(id).ok().and_then(|e| e.ctx.http.clone());
        let web = if reg.cap_on(WEB.id, Cap::LinksRead).await { gateway(WEB.id) } else { None };
        let printables = if reg.cap_on(PRINTABLES.id, Cap::LinksRead).await { gateway(PRINTABLES.id) } else { None };
        Ok(Self::with(web, printables, None))
    }

    /// Mit gegebenen Zugängen; `makerworld_base` ersetzt in Tests `https://makerworld.com`.
    pub fn with(web: Option<Arc<HttpGateway>>, printables: Option<Arc<HttpGateway>>, makerworld_base: Option<String>) -> Self {
        let mut resolvers: Vec<Box<dyn LinkResolver>> = Vec::new();
        if let (Some(api), Some(w)) = (printables, web.clone()) {
            resolvers.push(Box::new(Printables { api, web: w }));
        }
        if let Some(w) = web.clone() {
            resolvers.push(Box::new(MakerWorldLink { web: w.clone(), base: makerworld_base }));
            resolvers.push(Box::new(Thingiverse { web: w }));
        }
        Self { resolvers, web }
    }

    fn web(&self) -> Result<&HttpGateway> {
        self.web.as_deref().ok_or_else(|| AppError::Invalid("Die Anbindung „Web“ ist ausgeschaltet.".into()))
    }

    fn resolver(&self, source: &str) -> Option<&dyn LinkResolver> {
        self.resolvers.iter().find(|r| r.source() == source).map(|b| b.as_ref())
    }

    /// Welche Plattform steckt hinter dem Link? ("web", wenn keine passt)
    pub fn detect(&self, url: &str) -> (&'static str, &'static str, Option<String>) {
        self.resolvers
            .iter()
            .find_map(|r| r.id_in(url).map(|id| (r.source(), r.label(), Some(id))))
            .unwrap_or(("web", "Web", None))
    }

    pub async fn preview(&self, url: &str) -> Result<LinkPreview> {
        let url = url.trim();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(AppError::Invalid("Bitte einen Link mit http:// oder https:// angeben.".into()));
        }
        let (source, label, id) = self.detect(url);
        let mut p = match (self.resolver(source), id.as_deref()) {
            (Some(r), Some(id)) => r.preview(url, id).await?,
            _ => web_preview(self.web()?, url).await,
        };
        p.source = source.into();
        p.source_label = label.into();
        p.url = url.into();
        p.remote_id = id;
        if p.title.is_empty() {
            p.title = url.split("://").nth(1).unwrap_or(url).trim_end_matches('/').to_string();
        }
        Ok(p)
    }

    /// Datei herunterladen (Bytes), über den Resolver der Plattform.
    pub async fn download_file(&self, p: &LinkPreview, f: &RemoteFile) -> Result<Vec<u8>> {
        match self.resolver(&p.source) {
            Some(r) => r.download(p, f).await,
            None => Err(AppError::Invalid(format!("{} erlaubt keinen direkten Download.", p.source_label))),
        }
    }

    pub async fn download_image(&self, url: &str) -> Result<Vec<u8>> {
        Ok(self.web()?.get_bytes(url, None).await?)
    }
}

// ---------- Printables ----------

struct Printables {
    api: Arc<HttpGateway>,
    web: Arc<HttpGateway>,
}

#[async_trait]
impl LinkResolver for Printables {
    fn source(&self) -> &'static str {
        "printables"
    }
    fn label(&self) -> &'static str {
        "Printables"
    }
    fn id_in(&self, url: &str) -> Option<String> {
        capture(&RE_PRINTABLES, url)
    }

    async fn preview(&self, _url: &str, id: &str) -> Result<LinkPreview> {
        let q = "query($id:ID!){print(id:$id){id name summary description user{publicUsername} license{abbreviation name}
                 images{filePath} stls{id name fileSize} gcodes{id name fileSize} otherFiles{id name fileSize}}}";
        let v = self.api.query_json(PRINTABLES_GQL, &json!({"query": q, "variables": {"id": id}}), None).await?;
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

    /// Erst einen Download-Link anfordern (GraphQL), dann die Datei über den Web-Zugang laden.
    async fn download(&self, p: &LinkPreview, f: &RemoteFile) -> Result<Vec<u8>> {
        let q = "mutation($id:ID!,$printId:ID!,$fileType:DownloadFileTypeEnum!,$source:DownloadSourceEnum!){
                 getDownloadLink(id:$id,printId:$printId,fileType:$fileType,source:$source){ok output{link}}}";
        let vars = json!({"id": f.id, "printId": p.remote_id, "fileType": f.kind, "source": "model_detail"});
        let v = self.api.query_json(PRINTABLES_GQL, &json!({"query": q, "variables": vars}), None).await?;
        let link = v["data"]["getDownloadLink"]["output"]["link"]
            .as_str()
            .ok_or_else(|| AppError::Invalid(format!("Kein Download-Link für {}", f.name)))?;
        Ok(self.web.get_bytes(link, None).await?)
    }
}

// ---------- MakerWorld (nur Infos; Dateien über das Bambu-Konto) ----------

struct MakerWorldLink {
    web: Arc<HttpGateway>,
    /// nur für Tests; sonst makerworld.com bzw. makerworld.com.cn passend zum Link
    base: Option<String>,
}

#[async_trait]
impl LinkResolver for MakerWorldLink {
    fn source(&self) -> &'static str {
        "makerworld"
    }
    fn label(&self) -> &'static str {
        "MakerWorld"
    }
    fn id_in(&self, url: &str) -> Option<String> {
        capture(&RE_MAKERWORLD, url)
    }

    async fn preview(&self, url: &str, id: &str) -> Result<LinkPreview> {
        let base = match &self.base {
            Some(b) => b.as_str(),
            None if url.contains("makerworld.com.cn") => "https://makerworld.com.cn",
            None => "https://makerworld.com",
        };
        let v = self.web.get_json(&format!("{base}/api/v1/design-service/design/{id}"), None).await?;
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
}

// ---------- Thingiverse (Seitenvorschau, Dateien nur über die Webseite) ----------

struct Thingiverse {
    web: Arc<HttpGateway>,
}

#[async_trait]
impl LinkResolver for Thingiverse {
    fn source(&self) -> &'static str {
        "thingiverse"
    }
    fn label(&self) -> &'static str {
        "Thingiverse"
    }
    fn id_in(&self, url: &str) -> Option<String> {
        capture(&RE_THINGIVERSE, url)
    }
    async fn preview(&self, url: &str, _id: &str) -> Result<LinkPreview> {
        let mut p = web_preview(&self.web, url).await;
        if p.note.is_none() {
            p.note = Some("Thingiverse gibt Dateien nur über die Webseite heraus. Bitte dort herunterladen und hineinziehen.".into());
        }
        Ok(p)
    }
}

// ---------- allgemeine Seite ----------

/// OpenGraph-Angaben einer Seite. Fehler sind hier kein Abbruch, dann bleibt es beim Link.
async fn web_preview(web: &HttpGateway, url: &str) -> LinkPreview {
    let mut p = LinkPreview::default();
    let html = match web.get_text(url).await {
        Ok((s, t)) if (200..300).contains(&s) => t,
        Ok((s, _)) => {
            p.note = Some(format!("Die Seite hat keine Vorschau herausgegeben (HTTP {s}). Der Link wird trotzdem gespeichert."));
            return p;
        }
        Err(_) => {
            p.note = Some("Die Seite war nicht erreichbar. Der Link wird trotzdem gespeichert.".into());
            return p;
        }
    };
    let head = &html[..floor(&html, 200_000)];
    for m in RE_OG.captures_iter(head) {
        let content = m
            .get(0)
            .and_then(|tag| RE_CONTENT.captures(tag.as_str()))
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
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn links(printables_base: Option<&str>, mw: Option<String>) -> Links {
        let web = Some(Arc::new(HttpGateway::new(&WEB)));
        let p = printables_base.map(|b| Arc::new(HttpGateway::with_base(&PRINTABLES, b)));
        Links::with(web, p, mw)
    }

    #[test]
    fn plattform_erkennen() {
        let l = links(Some("http://127.0.0.1:9"), None);
        assert_eq!(l.detect("https://www.printables.com/model/3161-3d-benchy").0, "printables");
        assert_eq!(l.detect("https://www.printables.com/de/model/3161-3d-benchy").2.as_deref(), Some("3161"));
        assert_eq!(l.detect("https://makerworld.com/en/models/1000000-lens-cap#profileId-1").2.as_deref(), Some("1000000"));
        assert_eq!(l.detect("https://makerworld.com/de/models/42").1, "MakerWorld");
        assert_eq!(l.detect("https://www.thingiverse.com/thing:763622").2.as_deref(), Some("763622"));
        assert_eq!(l.detect("https://example.com/x").0, "web");
        // ohne Printables-Anbindung wird der Link zur allgemeinen Seite
        assert_eq!(links(None, None).detect("https://www.printables.com/model/3161").0, "web");
        assert_eq!(image_ext(&[0x89, b'P', b'N', b'G', 0]), "png");
    }

    #[tokio::test]
    async fn printables_vorschau_und_download() {
        let api = MockServer::start().await;
        let cdn = MockServer::start().await;
        Mock::given(method("POST")).and(path("/graphql/")).and(body_string_contains("getDownloadLink"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {"getDownloadLink": {"ok": true, "output": {"link": format!("{}/datei.stl", cdn.uri())}}}})))
            .mount(&api).await;
        Mock::given(method("POST")).and(path("/graphql/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {"print": {
                "id": "3161", "name": "3DBenchy", "summary": "Kurz", "description": "<p>Lang &amp; gut</p>",
                "user": {"publicUsername": "creative"}, "license": {"abbreviation": "CC BY", "name": "Namensnennung"},
                "images": [{"filePath": "media/x.png"}],
                "stls": [{"id": "1", "name": "benchy.stl", "fileSize": 100}], "gcodes": [], "otherFiles": []
            }}})))
            .mount(&api).await;
        Mock::given(method("GET")).and(path("/datei.stl")).respond_with(ResponseTemplate::new(200).set_body_bytes(b"solid x".to_vec())).mount(&cdn).await;

        let l = links(Some(&api.uri()), None);
        let p = l.preview("https://www.printables.com/model/3161-3d-benchy").await.unwrap();
        assert_eq!((p.source.as_str(), p.source_label.as_str(), p.remote_id.as_deref()), ("printables", "Printables", Some("3161")));
        assert_eq!((p.title.as_str(), p.designer.as_str(), p.license.as_str()), ("3DBenchy", "creative", "CC BY"));
        assert_eq!(p.description, "Lang & gut");
        assert_eq!(p.image_url.as_deref(), Some("https://media.printables.com/media/x.png"));
        assert_eq!(p.files.len(), 1);
        assert_eq!(l.download_file(&p, &p.files[0]).await.unwrap(), b"solid x");
    }

    #[tokio::test]
    async fn makerworld_thingiverse_und_web() {
        let s = MockServer::start().await;
        Mock::given(method("GET")).and(path("/api/v1/design-service/design/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 42, "title": "Vase", "summary": "<b>Schön</b>",
                "coverUrl": "https://cdn/x.jpg", "designCreator": {"name": "maker"}, "license": "BY",
                "instances": [{"title": "0.2 mm", "prediction": 3600, "weight": 50.5}]})))
            .mount(&s).await;
        Mock::given(method("GET")).and(path("/seite"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"<html><head><meta property="og:title" content="Halter &amp; Co"><meta name="description" content="Ein Halter"><meta property="og:image" content="https://img/x.png"></head></html>"#))
            .mount(&s).await;
        Mock::given(method("GET")).and(path("/kaputt")).respond_with(ResponseTemplate::new(503)).mount(&s).await;

        let l = links(None, Some(s.uri()));
        let m = l.preview("https://makerworld.com/de/models/42").await.unwrap();
        assert_eq!((m.title.as_str(), m.description.as_str(), m.designer.as_str()), ("Vase", "Schön", "maker"));
        assert_eq!(m.profiles[0].print_seconds, Some(3600));
        assert!(m.note.is_some());
        assert!(l.download_file(&m, &RemoteFile::default()).await.is_err(), "MakerWorld nur über das Konto");

        let w = l.preview(&format!("{}/seite", s.uri())).await.unwrap();
        assert_eq!((w.source.as_str(), w.title.as_str(), w.description.as_str()), ("web", "Halter & Co", "Ein Halter"));
        assert_eq!(w.image_url.as_deref(), Some("https://img/x.png"));
        let k = l.preview(&format!("{}/kaputt", s.uri())).await.unwrap();
        assert!(k.note.unwrap().contains("HTTP 503"));
        assert!(l.preview("ftp://x").await.is_err());
    }

    /// Echte Abrufe nur mit `PM_NET_TESTS=1`, damit `cargo test` offline läuft.
    #[tokio::test]
    async fn echte_abrufe() {
        if std::env::var("PM_NET_TESTS").is_err() {
            return;
        }
        let l = Links::with(Some(Arc::new(HttpGateway::new(&WEB))), Some(Arc::new(HttpGateway::new(&PRINTABLES))), None);
        let p = l.preview("https://www.printables.com/model/3161-3d-benchy").await.unwrap();
        assert_eq!(p.source, "printables");
        assert!(p.title.to_lowercase().contains("benchy"));
        assert!(p.image_url.is_some());
        let stl = p.files.iter().find(|f| f.name.ends_with(".stl")).unwrap();
        let bytes = l.download_file(&p, stl).await.unwrap();
        assert!(bytes.len() > 1_000_000);

        let m = l.preview("https://makerworld.com/en/models/1000000").await.unwrap();
        assert_eq!(m.source, "makerworld");
        assert!(!m.title.is_empty() && m.image_url.is_some() && !m.profiles.is_empty());
        assert!(l.download_image(m.image_url.as_deref().unwrap()).await.unwrap().len() > 1000);
    }
}
