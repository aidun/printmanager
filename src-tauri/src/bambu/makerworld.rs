//! MakerWorld im Katalog (Stufe 4): Sammlungen und ihre Modelle übernehmen, 3MF erst bei Bedarf laden.
//! Nur lesen. Die App ändert nichts an deinen Sammlungen. Dateien sind für deinen eigenen Gebrauch.

use super::Bambu;
use crate::commands::{catalog_dir, describe_catalog_file, AppState};
use crate::error::{AppError, Result};
use crate::model::{MwCollection, MwDesign, MwSyncResult, NewCatalogEntry};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

const COLLECTIONS: &str = "/v1/design-service/my/favorites/listlite";

fn id_of(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Liste der Sammlungen: `default` plus `hits`.
pub fn parse_collections(v: &Value) -> Vec<MwCollection> {
    let one = |c: &Value| -> Option<MwCollection> {
        Some(MwCollection {
            id: id_of(&c["id"])?,
            title: c["title"].as_str().unwrap_or("Sammlung").trim().to_string(),
            design_cnt: c["designCnt"].as_i64().unwrap_or(0),
            is_default: c["isDefault"].as_bool().unwrap_or(false) as i64,
            enabled: 1,
            synced_at: None,
        })
    };
    let mut out: Vec<MwCollection> = Vec::new();
    for c in std::iter::once(&v["default"]).chain(v["hits"].as_array().into_iter().flatten()) {
        if let Some(c) = one(c) {
            if !out.iter().any(|x| x.id == c.id) {
                out.push(c);
            }
        }
    }
    out
}

/// Modelle aus `favorites/{id}` (Feld `designs`) oder `favorites/{id}/designs` (Feld `hits`).
pub fn parse_designs(v: &Value) -> Vec<MwDesign> {
    v["designs"]
        .as_array()
        .or(v["hits"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|d| {
            Some(MwDesign {
                id: id_of(&d["id"])?,
                title: d["title"].as_str().unwrap_or("").trim().to_string(),
                designer: d["designCreator"]["name"].as_str().unwrap_or("").to_string(),
                cover: d["cover"].as_str().filter(|s| s.starts_with("https://")).map(str::to_string),
                slug: d["slug"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Alle Modelle einer Sammlung: erst alles auf einmal, sonst seitenweise (20 je Seite, `total` zählt).
async fn collection_designs(bambu: &Bambu, id: &str) -> Result<Vec<MwDesign>> {
    let full = bambu.get(&format!("/v1/design-service/favorites/{id}")).await?;
    let mut list = parse_designs(&full);
    let expected = full["designCnt"].as_i64().unwrap_or(0) - full["hiddenCnt"].as_i64().unwrap_or(0);
    if (list.len() as i64) < expected {
        list.clear();
        let mut offset = 0;
        loop {
            let page = bambu.get(&format!("/v1/design-service/favorites/{id}/designs?limit=20&offset={offset}")).await?;
            let got = parse_designs(&page);
            let total = page["total"].as_i64().unwrap_or(0);
            let n = got.len();
            list.extend(got);
            offset += 20;
            if n == 0 || offset >= total || offset > 2000 {
                break;
            }
        }
    }
    Ok(list)
}

/// Sammlungsliste holen und alle eingeschalteten Sammlungen in den Katalog übernehmen.
pub async fn sync(app: &AppHandle) -> Result<MwSyncResult> {
    let bambu = app.state::<Bambu>();
    let state = app.state::<AppState>();
    let collections = parse_collections(&bambu.get(COLLECTIONS).await?);
    {
        let ctx = state.ctx.read().await;
        ctx.store.save_mw_collections(&collections).await?;
    }
    let enabled: Vec<MwCollection> = {
        let ctx = state.ctx.read().await;
        ctx.store.list_mw_collections().await?.into_iter().filter(|c| c.enabled != 0).collect()
    };
    let mut r = MwSyncResult { collections: enabled.len() as i64, ..Default::default() };
    let mut complete = true;
    for c in &enabled {
        match collection_designs(&bambu, &c.id).await {
            Ok(designs) => {
                r.designs += designs.len() as i64;
                let ctx = state.ctx.read().await;
                r.added += ctx.store.save_mw_designs(&c.id, &designs).await?;
            }
            Err(e) => {
                complete = false;
                eprintln!("MakerWorld-Sammlung {} nicht abrufbar: {e}", c.title);
            }
        }
    }
    if complete {
        let ctx = state.ctx.read().await;
        r.missing = ctx.store.mark_mw_missing().await?;
    }
    bambu.status.write().await.mw_synced_at = Some(crate::store::now());
    Ok(r)
}

pub fn sync_messages(r: &MwSyncResult) -> Vec<String> {
    if r.added > 0 {
        vec![format!("{} neue Modelle aus MakerWorld im Katalog", r.added)]
    } else {
        vec![]
    }
}

/// Druckprofil wählen: Standardprofil, sonst das erste.
fn instance_of(design: &Value) -> Option<String> {
    id_of(&design["defaultInstanceId"])
        .filter(|s| s != "0")
        .or_else(|| design["instances"].as_array().and_then(|a| a.first()).and_then(|i| id_of(&i["id"])))
}

/// 3MF eines MakerWorld-Eintrags laden (bei Bedarf) und den Katalogeintrag damit vervollständigen.
pub async fn fetch_file(app: &AppHandle, catalog_id: &str) -> Result<()> {
    let bambu = app.state::<Bambu>();
    let state = app.state::<AppState>();
    let (remote, dir) = {
        let ctx = state.ctx.read().await;
        let e = ctx.store.get_catalog(catalog_id).await?;
        if !e.file_name.is_empty() {
            return Ok(());
        }
        let remote = e.remote_id.ok_or_else(|| AppError::Invalid("Kein MakerWorld-Eintrag.".into()))?;
        (remote, catalog_dir(&ctx, catalog_id))
    };
    let design = bambu.get(&format!("/v1/design-service/design/{remote}")).await?;
    let instance = instance_of(&design).ok_or_else(|| AppError::Invalid("Für dieses Modell gibt es kein Druckprofil.".into()))?;
    let f = bambu
        .get(&format!("/v1/design-service/instance/{instance}/f3mf?type=download"))
        .await
        .map_err(|_| AppError::Invalid("MakerWorld gibt die Datei nicht heraus (z.B. kostenpflichtig oder nur in Bambu Handy).".into()))?;
    let url = f["url"].as_str().filter(|u| u.starts_with("https://")).ok_or_else(|| AppError::Invalid("MakerWorld liefert keine Download-Adresse.".into()))?;
    let name = f["name"].as_str().unwrap_or("modell.3mf");
    let name = crate::files::safe_name(if name.to_lowercase().ends_with(".3mf") { name } else { "modell.3mf" });
    let bytes = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| AppError::Invalid(format!("Download fehlgeschlagen: {}", e.without_url())))?
        .bytes()
        .await
        .map_err(|e| AppError::Invalid(format!("Download fehlgeschlagen: {}", e.without_url())))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(&name), &bytes)?;
    let defaults = NewCatalogEntry {
        description: crate::threemf::html_to_text(design["summary"].as_str().unwrap_or("")),
        designer: design["designCreator"]["name"].as_str().unwrap_or("").to_string(),
        license: design["license"].as_str().unwrap_or("").to_string(),
        ..Default::default()
    };
    let e = describe_catalog_file(&dir, &name, defaults)?;
    let ctx = state.ctx.read().await;
    ctx.store.set_catalog_file(catalog_id, &instance, &e).await
}

// ---------- Befehle ----------

#[tauri::command]
pub async fn mw_collections(app: AppHandle) -> Result<Vec<MwCollection>> {
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    ctx.store.list_mw_collections().await
}

#[tauri::command]
pub async fn mw_collection_toggle(app: AppHandle, id: String, enabled: bool) -> Result<()> {
    {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        ctx.store.set_mw_collection_enabled(&id, enabled).await?;
        if !enabled {
            ctx.store.mark_mw_missing().await?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn mw_sync(app: AppHandle, bambu: State<'_, Bambu>) -> Result<MwSyncResult> {
    if !bambu.status().await.connected {
        return Err(AppError::Invalid("Nicht am Bambu-Konto angemeldet.".into()));
    }
    sync(&app).await
}

#[tauri::command]
pub async fn catalog_fetch_file(app: AppHandle, id: String) -> Result<()> {
    fetch_file(&app, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sammlungen_und_modelle_lesen() {
        let v = serde_json::json!({
            "default": {"id": 40174759, "title": "Weihnachten", "designCnt": 9, "isDefault": true},
            "hits": [{"id": 14622613, "title": "Default Collection", "designCnt": 170, "isDefault": false},
                     {"id": 40174759, "title": "Weihnachten", "designCnt": 9, "isDefault": true}],
            "total": 23});
        let c = parse_collections(&v);
        assert_eq!(c.len(), 2, "Standard nicht doppelt");
        assert_eq!((c[0].id.as_str(), c[0].is_default, c[1].design_cnt), ("40174759", 1, 170));

        let d = serde_json::json!({"designs": [
            {"id": 3338545, "title": "Christmas Tree", "cover": "https://makerworld.bblmw.com/x.png",
             "designCreator": {"name": "NowCraft3D"}, "slug": "christmas-tree"},
            {"id": null}]});
        let l = parse_designs(&d);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].id.as_str(), l[0].designer.as_str()), ("3338545", "NowCraft3D"));
        assert_eq!(parse_designs(&serde_json::json!({"hits": [{"id": "7", "cover": "http://unsicher"}]}))[0].cover, None);

        assert_eq!(instance_of(&serde_json::json!({"defaultInstanceId": 3554696})).as_deref(), Some("3554696"));
        assert_eq!(instance_of(&serde_json::json!({"defaultInstanceId": 0, "instances": [{"id": 5}]})).as_deref(), Some("5"));
    }
}
