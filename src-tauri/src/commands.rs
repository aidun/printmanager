//! Befehle, die die Oberfläche per `invoke()` aufruft.

use crate::error::{AppError, Result};
use crate::files;
use crate::linkimport::LinkPreview;
use crate::model::*;
use crate::settings::Settings;
use crate::store::Store;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{AppHandle, Manager, State};
use tokio::sync::RwLock;

pub struct Ctx {
    pub store: Store,
    pub data_dir: PathBuf,
    pub settings: Settings,
}

pub struct AppState {
    pub ctx: RwLock<Ctx>,
    pub settings_file: PathBuf,
    pub default_data_dir: PathBuf,
    /// Standardort von Bambu Studios `spools.json`
    pub default_spools_file: PathBuf,
    pub watcher: std::sync::Mutex<Option<notify::RecommendedWatcher>>,
}

impl AppState {
    pub fn spools_file(&self, ctx: &Ctx) -> PathBuf {
        ctx.settings
            .spools_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_spools_file.clone())
    }
}

/// Datenbank öffnen (bzw. anlegen) und den Datenordner für die Bildanzeige freigeben.
pub async fn open_ctx(app: &AppHandle, settings: Settings, default_dir: &Path) -> Result<Ctx> {
    let data_dir = settings.data_dir(default_dir);
    std::fs::create_dir_all(data_dir.join("orders"))?;
    let url = match settings.database_url.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
        Some(u) => u.to_string(),
        None => Store::sqlite_url(&data_dir.join("printmanager.db")),
    };
    let store = Store::connect(&url).await?;
    let _ = app.asset_protocol_scope().allow_directory(&data_dir, true);
    Ok(Ctx { store, data_dir, settings })
}

async fn order_folder(ctx: &Ctx, order_id: &str) -> Result<PathBuf> {
    let o = ctx.store.get_order(order_id).await?;
    Ok(files::order_dir(&ctx.data_dir, o.number))
}

// ---------- App ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    data_dir: String,
    default_data_dir: String,
    database: String,
    settings: Settings,
    statuses: Vec<&'static str>,
    studio_found: bool,
    spools_file: String,
    default_spools_file: String,
    version: String,
}

#[tauri::command]
pub async fn app_info(app: AppHandle, state: State<'_, AppState>) -> Result<AppInfo> {
    let ctx = state.ctx.read().await;
    let database = match ctx.settings.database_url.as_deref().filter(|u| !u.trim().is_empty()) {
        Some(u) => u.split('@').next_back().unwrap_or(u).to_string(), // ohne Zugangsdaten
        None => ctx.data_dir.join("printmanager.db").display().to_string(),
    };
    Ok(AppInfo {
        data_dir: ctx.data_dir.display().to_string(),
        default_data_dir: state.default_data_dir.display().to_string(),
        database,
        settings: ctx.settings.clone(),
        statuses: STATUSES.to_vec(),
        studio_found: studio_path(&ctx.settings).is_some(),
        spools_file: state.spools_file(&ctx).display().to_string(),
        default_spools_file: state.default_spools_file.display().to_string(),
        version: app.package_info().version.to_string(),
    })
}

#[tauri::command]
pub async fn settings_save(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<()> {
    let new_ctx = open_ctx(&app, settings.clone(), &state.default_data_dir).await?;
    settings.save(&state.settings_file)?;
    let (orders, spools) = (new_ctx.data_dir.join("orders"), state.spools_file(&new_ctx));
    *state.ctx.write().await = new_ctx;
    crate::watch::restart(&app, orders, spools);
    Ok(())
}

// ---------- Personen ----------

#[tauri::command]
pub async fn people_list(state: State<'_, AppState>) -> Result<Vec<Person>> {
    state.ctx.read().await.store.list_people().await
}

#[tauri::command]
pub async fn person_save(state: State<'_, AppState>, person: PersonInput) -> Result<String> {
    state.ctx.read().await.store.save_person(person).await
}

#[tauri::command]
pub async fn person_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    state.ctx.read().await.store.delete_person(&id).await
}

// ---------- Aufträge ----------

#[tauri::command]
pub async fn orders_list(state: State<'_, AppState>, search: Option<String>) -> Result<Vec<OrderSummary>> {
    state.ctx.read().await.store.list_orders(search.as_deref()).await
}

#[tauri::command]
pub async fn order_get(state: State<'_, AppState>, id: String) -> Result<OrderDetail> {
    let ctx = state.ctx.read().await;
    let folder = order_folder(&ctx, &id).await?;
    ctx.store.order_detail(&id, folder.display().to_string()).await
}

#[tauri::command]
pub async fn order_create(state: State<'_, AppState>, input: OrderInput) -> Result<Order> {
    let ctx = state.ctx.read().await;
    let o = ctx.store.create_order(input).await?;
    std::fs::create_dir_all(files::order_dir(&ctx.data_dir, o.number))?;
    Ok(o)
}

#[tauri::command]
pub async fn order_update(state: State<'_, AppState>, update: OrderUpdate) -> Result<()> {
    state.ctx.read().await.store.update_order(update).await
}

#[tauri::command]
pub async fn order_set_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
    note: Option<String>,
) -> Result<()> {
    state.ctx.read().await.store.set_status(&id, &status, note.as_deref()).await
}

#[tauri::command]
pub async fn order_move(state: State<'_, AppState>, id: String, direction: i64) -> Result<()> {
    state.ctx.read().await.store.move_order(&id, direction).await
}

/// Löscht den Auftrag samt Ordner. Der Ordner wandert in den Papierkorb-Ordner `trash/` im Datenordner.
#[tauri::command]
pub async fn order_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    let ctx = state.ctx.read().await;
    let folder = order_folder(&ctx, &id).await?;
    ctx.store.delete_order(&id).await?;
    if folder.exists() {
        let trash = ctx.data_dir.join("trash");
        std::fs::create_dir_all(&trash)?;
        let name = format!("{}-{}", folder.file_name().unwrap().to_string_lossy(), crate::store::now().replace(':', "-"));
        std::fs::rename(&folder, trash.join(name))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn note_add(state: State<'_, AppState>, order_id: String, body: String) -> Result<()> {
    state.ctx.read().await.store.add_note(&order_id, &body).await
}

// ---------- Teile ----------

#[tauri::command]
pub async fn item_save(state: State<'_, AppState>, item: Item) -> Result<String> {
    state.ctx.read().await.store.save_item(item).await
}

#[tauri::command]
pub async fn item_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    state.ctx.read().await.store.delete_item(&id).await
}

// ---------- Anhänge ----------

pub(crate) async fn add_file_attachment(ctx: &Ctx, order_id: &str, name: &str) -> Result<String> {
    let kind = if files::is_image(name) { "image" } else { "file" };
    ctx.store.add_attachment(order_id, kind, name, Some(name), None, None).await
}

/// Dateien (z.B. per Drag&Drop) in den Auftragsordner kopieren.
#[tauri::command]
pub async fn attachment_add_paths(state: State<'_, AppState>, order_id: String, paths: Vec<String>) -> Result<usize> {
    let ctx = state.ctx.read().await;
    let dir = order_folder(&ctx, &order_id).await?;
    let mut n = 0;
    for p in paths {
        let name = files::copy_into(&dir, Path::new(&p))?;
        add_file_attachment(&ctx, &order_id, &name).await?;
        n += 1;
    }
    Ok(n)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Eingefügte Bilder (Zwischenablage) als rohe Bytes. Header: `x-order-id`, `x-name` (URI-kodiert).
#[tauri::command]
pub async fn attachment_add_bytes(state: State<'_, AppState>, request: tauri::ipc::Request<'_>) -> Result<String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(AppError::Invalid("Erwarte Binärdaten".into()));
    };
    let header = |k: &str| request.headers().get(k).and_then(|v| v.to_str().ok()).map(percent_decode);
    let order_id = header("x-order-id").ok_or_else(|| AppError::Invalid("x-order-id fehlt".into()))?;
    let name = header("x-name").unwrap_or_else(|| "eingefuegt.png".into());
    let ctx = state.ctx.read().await;
    let dir = order_folder(&ctx, &order_id).await?;
    let name = files::write_into(&dir, &name, bytes)?;
    add_file_attachment(&ctx, &order_id, &name).await
}

#[tauri::command]
pub async fn attachment_add_link(
    state: State<'_, AppState>,
    order_id: String,
    url: String,
    name: Option<String>,
) -> Result<String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Invalid("Links müssen mit http:// oder https:// beginnen.".into()));
    }
    let name = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| link_label(url));
    state.ctx.read().await.store.add_attachment(&order_id, "link", &name, None, Some(url), None).await
}

/// Kurzer Name für einen Link: Host ohne `www.`
fn link_label(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).trim_start_matches("www.").to_string()
}

#[tauri::command]
pub async fn attachment_add_text(state: State<'_, AppState>, order_id: String, body: String) -> Result<String> {
    let body = body.trim();
    if body.is_empty() {
        return Err(AppError::Invalid("Der Text ist leer.".into()));
    }
    let name: String = body.lines().next().unwrap_or("").chars().take(60).collect();
    state.ctx.read().await.store.add_attachment(&order_id, "text", &name, None, None, Some(body)).await
}

#[tauri::command]
pub async fn attachment_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    let ctx = state.ctx.read().await;
    let a = ctx.store.get_attachment(&id).await?;
    ctx.store.delete_attachment(&id).await?;
    if let Some(rel) = &a.path {
        let dir = order_folder(&ctx, &a.order_id).await?;
        let _ = std::fs::remove_file(files::resolve_in(&dir, rel)?);
    }
    Ok(())
}

// ---------- Öffnen im System ----------

async fn attachment_file(ctx: &Ctx, id: &str) -> Result<PathBuf> {
    let a = ctx.store.get_attachment(id).await?;
    let rel = a.path.ok_or_else(|| AppError::Invalid("Der Anhang ist keine Datei.".into()))?;
    let dir = order_folder(ctx, &a.order_id).await?;
    files::resolve_in(&dir, &rel)
}

fn spawn(cmd: &mut Command) -> Result<()> {
    cmd.spawn().map(|_| ()).map_err(|e| AppError::Invalid(format!("Konnte nicht öffnen: {e}")))
}

/// Datei mit dem Standardprogramm öffnen.
fn open_default(target: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return spawn(Command::new("open").arg(target));
    #[cfg(target_os = "windows")]
    return spawn(Command::new("explorer").arg(target));
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    return spawn(Command::new("xdg-open").arg(target));
}

pub fn studio_path(settings: &Settings) -> Option<PathBuf> {
    if let Some(p) = settings.bambu_studio_path.as_deref().filter(|p| !p.trim().is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.exists());
    }
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["/Applications/BambuStudio.app", "/Applications/Bambu Studio.app"]
    } else if cfg!(target_os = "windows") {
        &[r"C:\Program Files\Bambu Studio\bambu-studio.exe"]
    } else {
        &["/usr/bin/bambu-studio", "/usr/local/bin/bambu-studio"]
    };
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

#[tauri::command]
pub async fn attachment_open(state: State<'_, AppState>, id: String) -> Result<()> {
    let ctx = state.ctx.read().await;
    open_default(&attachment_file(&ctx, &id).await?.display().to_string())
}

fn launch_studio(settings: &Settings, file: &Path) -> Result<()> {
    let studio = studio_path(settings)
        .ok_or_else(|| AppError::Invalid("Bambu Studio wurde nicht gefunden. Pfad in den Einstellungen angeben.".into()))?;
    if cfg!(target_os = "macos") {
        spawn(Command::new("open").arg("-a").arg(&studio).arg(file))
    } else {
        spawn(Command::new(&studio).arg(file))
    }
}

/// Druckdatei eines Auftrags in Bambu Studio öffnen. Profile und Filamente wählst du dort.
/// Steht der Auftrag noch vor „In Studio“, rückt er auf „In Studio“ vor. Rückgabe: neuer Status, falls geändert.
async fn open_attachment_in_studio(ctx: &Ctx, attachment_id: &str) -> Result<Option<String>> {
    let a = ctx.store.get_attachment(attachment_id).await?;
    let file = attachment_file(ctx, attachment_id).await?;
    launch_studio(&ctx.settings, &file)?;
    let o = ctx.store.get_order(&a.order_id).await?;
    if ["eingang", "klaeren", "vorbereiten"].contains(&o.status.as_str()) {
        ctx.store.set_status(&o.id, "in_studio", Some("In Bambu Studio geöffnet")).await?;
        return Ok(Some("in_studio".into()));
    }
    Ok(None)
}

#[tauri::command]
pub async fn attachment_open_in_studio(state: State<'_, AppState>, id: String) -> Result<Option<String>> {
    let ctx = state.ctx.read().await;
    open_attachment_in_studio(&ctx, &id).await
}

#[tauri::command]
pub async fn order_reveal(state: State<'_, AppState>, id: String) -> Result<()> {
    let ctx = state.ctx.read().await;
    let dir = order_folder(&ctx, &id).await?;
    std::fs::create_dir_all(&dir)?;
    open_default(&dir.display().to_string())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Invalid("Nur http- und https-Links".into()));
    }
    #[cfg(target_os = "windows")]
    return spawn(Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]));
    #[cfg(not(target_os = "windows"))]
    open_default(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilfsfunktionen() {
        assert_eq!(percent_decode("Bild%20%C3%A4.png"), "Bild ä.png");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(link_label("https://www.makerworld.com/de/models/1"), "makerworld.com");
    }
}

// ---------- Katalog ----------

pub(crate) fn catalog_dir(ctx: &Ctx, id: &str) -> PathBuf {
    ctx.data_dir.join("catalog").join(files::safe_name(id))
}

/// Datei im Ordner `dir` als Katalogprojekt erfassen (3MF wird ausgewertet).
async fn register_catalog(
    ctx: &Ctx,
    id: &str,
    dir: &Path,
    file_name: &str,
    defaults: crate::model::NewCatalogEntry,
) -> Result<()> {
    let e = describe_catalog_file(dir, file_name, defaults)?;
    ctx.store.insert_catalog(id, e).await
}

/// Angaben zu einer Katalogdatei: 3MF auswerten (Vorschau, Platten, Filamente, Druckzeit), Lücken aus `defaults`.
pub(crate) fn describe_catalog_file(
    dir: &Path,
    file_name: &str,
    defaults: crate::model::NewCatalogEntry,
) -> Result<crate::model::NewCatalogEntry> {
    let mut e = defaults;
    e.file_name = file_name.to_string();
    if file_name.to_lowercase().ends_with(".3mf") {
        let info = crate::threemf::analyze(&dir.join(file_name))?;
        if let Some(t) = &info.thumbnail {
            std::fs::write(dir.join("vorschau.png"), t)?;
            e.thumb_name = Some("vorschau.png".into());
        }
        if !info.title.is_empty() && e.title.is_empty() {
            e.title = info.title.clone();
        }
        if e.description.is_empty() {
            e.description = info.description.clone();
        }
        if e.designer.is_empty() {
            e.designer = info.designer.clone();
        }
        if e.license.is_empty() {
            e.license = info.license.clone();
        }
        e.plates = info.plates;
        e.print_seconds = info.print_seconds;
        e.weight_grams = info.weight_grams.map(|g| g.round() as i64);
        e.filaments = serde_json::to_string(&info.filaments).unwrap_or_else(|_| "[]".into());
    }
    if e.title.is_empty() {
        e.title = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or(file_name).replace(['_', '-'], " ");
    }
    if e.filaments.is_empty() {
        e.filaments = "[]".into();
    }
    Ok(e)
}

#[tauri::command]
pub async fn catalog_list(state: State<'_, AppState>, include_hidden: bool) -> Result<Vec<CatalogEntry>> {
    state.ctx.read().await.store.list_catalog(include_hidden).await
}

/// 3MF-, STL- oder STEP-Dateien in den Katalog übernehmen. Liefert die IDs der neuen Projekte.
#[tauri::command]
pub async fn catalog_import_paths(state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<String>> {
    let ctx = state.ctx.read().await;
    let mut ids = vec![];
    for p in paths {
        let src = Path::new(&p);
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !is_print_file(name) {
            return Err(AppError::Invalid(format!("„{name}“ ist keine 3MF-, STL- oder STEP-Datei.")));
        }
        let id = Store::new_catalog_id();
        let dir = catalog_dir(&ctx, &id);
        let file = files::copy_into(&dir, src)?;
        if let Err(e) = register_catalog(&ctx, &id, &dir, &file, Default::default()).await {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        ids.push(id);
    }
    Ok(ids)
}

fn is_print_file(name: &str) -> bool {
    let l = name.to_lowercase();
    [".3mf", ".stl", ".step", ".stp", ".obj"].iter().any(|e| l.ends_with(e))
}

#[tauri::command]
pub async fn catalog_update(state: State<'_, AppState>, update: CatalogUpdate) -> Result<()> {
    state.ctx.read().await.store.update_catalog(update).await
}

#[tauri::command]
pub async fn catalog_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    let ctx = state.ctx.read().await;
    ctx.store.delete_catalog(&id).await?;
    let dir = catalog_dir(&ctx, &id);
    if dir.exists() {
        let trash = ctx.data_dir.join("trash");
        std::fs::create_dir_all(&trash)?;
        std::fs::rename(&dir, trash.join(format!("katalog-{id}")))?;
    }
    Ok(())
}

/// Katalogprojekt selbst in Bambu Studio öffnen (zum Pflegen der Vorlage).
#[tauri::command]
pub async fn catalog_open_in_studio(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    // MakerWorld-Eintrag ohne Datei: 3MF jetzt laden
    crate::bambu::makerworld::fetch_file(&app, &id).await?;
    let ctx = state.ctx.read().await;
    let e = ctx.store.get_catalog(&id).await?;
    launch_studio(&ctx.settings, &files::resolve_in(&catalog_dir(&ctx, &id), &e.file_name)?)
}

/// Katalogprojekt als Teil in einen Auftrag übernehmen: Arbeitskopie der Datei im Auftragsordner + Teil.
#[tauri::command]
pub async fn catalog_add_to_order(
    app: AppHandle,
    state: State<'_, AppState>,
    order_id: String,
    catalog_id: String,
    quantity: i64,
    color_wish: String,
) -> Result<String> {
    crate::bambu::makerworld::fetch_file(&app, &catalog_id).await?;
    let ctx = state.ctx.read().await;
    let e = ctx.store.get_catalog(&catalog_id).await?;
    let src = files::resolve_in(&catalog_dir(&ctx, &catalog_id), &e.file_name)?;
    let dir = order_folder(&ctx, &order_id).await?;
    let name = files::copy_into(&dir, &src)?;
    let att = ctx.store.add_attachment(&order_id, "file", &name, Some(&name), None, None).await?;
    if let Some(t) = &e.thumb_name {
        if let Ok(thumb) = files::resolve_in(&catalog_dir(&ctx, &catalog_id), t) {
            let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("vorschau");
            let ext = Path::new(t).extension().and_then(|e| e.to_str()).unwrap_or("png");
            let img = files::write_into(&dir, &format!("{stem}-vorschau.{ext}"), &std::fs::read(thumb)?)?;
            ctx.store.add_attachment(&order_id, "image", &img, Some(&img), None, None).await?;
        }
    }
    ctx.store
        .save_item(Item {
            id: String::new(),
            order_id,
            position: 0,
            title: e.title,
            quantity,
            color_wish,
            note: String::new(),
            catalog_id: Some(catalog_id),
            done: 0,
            file_attachment_id: Some(att),
        })
        .await
}

// ---------- Links von Modell-Plattformen ----------

#[tauri::command]
pub async fn link_preview(url: String) -> Result<LinkPreview> {
    crate::linkimport::preview(&url).await
}

/// Link samt Vorschaubild, Beschreibung und gewählten Dateien an einen Auftrag hängen.
/// Heruntergeladene Druckdateien werden zusätzlich als Teile angelegt.
#[tauri::command]
pub async fn link_import_to_order(
    state: State<'_, AppState>,
    order_id: String,
    preview: LinkPreview,
    file_ids: Vec<String>,
) -> Result<usize> {
    // Downloads ohne Sperre, damit die App währenddessen bedienbar bleibt
    let image = match &preview.image_url {
        Some(u) => crate::linkimport::download_image(u).await.ok(),
        None => None,
    };
    let mut downloads = vec![];
    for f in preview.files.iter().filter(|f| file_ids.contains(&f.id)) {
        downloads.push((f.name.clone(), crate::linkimport::download_file(&preview, f).await?));
    }

    let ctx = state.ctx.read().await;
    let dir = order_folder(&ctx, &order_id).await?;
    let title = format!("{}: {}", preview.source_label, preview.title);
    ctx.store.add_attachment(&order_id, "link", &title, None, Some(&preview.url), None).await?;
    let mut n = 1;
    if let Some(img) = image {
        let name = files::write_into(&dir, &format!("{}-vorschau.{}", preview.source, crate::linkimport::image_ext(&img)), &img)?;
        ctx.store.add_attachment(&order_id, "image", &name, Some(&name), None, None).await?;
        n += 1;
    }
    let mut text = preview.description.chars().take(3000).collect::<String>();
    if !preview.designer.is_empty() || !preview.license.is_empty() {
        text = format!("{}\n\nDesigner: {} · Lizenz: {}", text, preview.designer, preview.license).trim().to_string();
    }
    if !text.trim().is_empty() {
        ctx.store.add_attachment(&order_id, "text", &preview.title, None, None, Some(&text)).await?;
        n += 1;
    }
    for (name, bytes) in downloads {
        let name = files::write_into(&dir, &name, &bytes)?;
        let att = ctx.store.add_attachment(&order_id, "file", &name, Some(&name), None, None).await?;
        if is_print_file(&name) {
            let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or(&name).to_string();
            ctx.store
                .save_item(Item {
                    id: String::new(),
                    order_id: order_id.clone(),
                    position: 0,
                    title: stem,
                    quantity: 1,
                    color_wish: String::new(),
                    note: String::new(),
                    catalog_id: None,
                    done: 0,
                    file_attachment_id: Some(att),
                })
                .await?;
        }
        n += 1;
    }
    Ok(n)
}

/// Dateien eines Links (Printables) als Katalogprojekte übernehmen.
#[tauri::command]
pub async fn catalog_import_link(state: State<'_, AppState>, preview: LinkPreview, file_ids: Vec<String>) -> Result<Vec<String>> {
    let chosen: Vec<_> = preview.files.iter().filter(|f| file_ids.contains(&f.id) && is_print_file(&f.name)).collect();
    if chosen.is_empty() {
        return Err(AppError::Invalid("Bitte mindestens eine 3MF- oder STL-Datei auswählen.".into()));
    }
    let image = match &preview.image_url {
        Some(u) => crate::linkimport::download_image(u).await.ok(),
        None => None,
    };
    let mut downloads = vec![];
    for f in &chosen {
        downloads.push((f.name.clone(), crate::linkimport::download_file(&preview, f).await?));
    }
    let ctx = state.ctx.read().await;
    let mut ids = vec![];
    let many = downloads.len() > 1;
    for (name, bytes) in downloads {
        let id = Store::new_catalog_id();
        let dir = catalog_dir(&ctx, &id);
        let file = files::write_into(&dir, &name, &bytes)?;
        let mut defaults = crate::model::NewCatalogEntry {
            title: if many {
                format!("{} – {}", preview.title, Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or(&name))
            } else {
                preview.title.clone()
            },
            description: preview.description.chars().take(5000).collect(),
            source_url: Some(preview.url.clone()),
            designer: preview.designer.clone(),
            license: preview.license.clone(),
            ..Default::default()
        };
        if let Some(img) = &image {
            let thumb = files::write_into(&dir, &format!("bild.{}", crate::linkimport::image_ext(img)), img)?;
            defaults.thumb_name = Some(thumb);
        }
        // Eine 3MF bringt ihr eigenes Vorschaubild mit, das hat Vorrang (register_catalog überschreibt thumb_name)
        if let Err(e) = register_catalog(&ctx, &id, &dir, &file, defaults).await {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        ids.push(id);
    }
    Ok(ids)
}

// ---------- Filament und Drucke ----------

#[tauri::command]
pub async fn spools_list(state: State<'_, AppState>) -> Result<Vec<Spool>> {
    state.ctx.read().await.store.list_spools().await
}

/// Spulen erneut aus Bambu Studio einlesen.
#[tauri::command]
pub async fn spools_sync(state: State<'_, AppState>) -> Result<SyncResult> {
    let ctx = state.ctx.read().await;
    let file = state.spools_file(&ctx);
    let mut r = crate::watch::sync_spools(&ctx, &file).await?;
    r.found = file.is_file();
    Ok(r)
}

#[tauri::command]
pub async fn spool_update(state: State<'_, AppState>, update: SpoolUpdate) -> Result<()> {
    state.ctx.read().await.store.update_spool(update).await
}

#[tauri::command]
pub async fn run_set_result(state: State<'_, AppState>, id: String, result: String, note: Option<String>) -> Result<()> {
    state.ctx.read().await.store.set_run_result(&id, &result, note.as_deref().unwrap_or("")).await
}

#[tauri::command]
pub async fn run_deduct(state: State<'_, AppState>, id: String, deductions: Vec<Deduction>) -> Result<()> {
    state.ctx.read().await.store.deduct_run(&id, &deductions).await
}

#[tauri::command]
pub async fn run_undo(state: State<'_, AppState>, id: String) -> Result<()> {
    state.ctx.read().await.store.undo_deduct(&id).await
}

#[tauri::command]
pub async fn run_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    state.ctx.read().await.store.delete_run(&id).await
}

/// Auftragsordner von Hand nach gesliced Dateien durchsuchen.
#[tauri::command]
pub async fn order_scan(state: State<'_, AppState>, id: String) -> Result<Vec<String>> {
    let ctx = state.ctx.read().await;
    let dir = order_folder(&ctx, &id).await?;
    crate::watch::scan_order(&ctx, &dir).await
}

// ---------- Archiv und Statistik ----------

#[tauri::command]
pub async fn order_archive(state: State<'_, AppState>, id: String, archived: bool) -> Result<()> {
    state.ctx.read().await.store.set_archived(&id, archived).await
}

/// Übergebene und abgelehnte Aufträge archivieren, die seit `days` Tagen unverändert sind.
#[tauri::command]
pub async fn orders_archive_closed(state: State<'_, AppState>, days: i64) -> Result<u64> {
    state.ctx.read().await.store.archive_closed(days.max(0)).await
}

#[tauri::command]
pub async fn stats(state: State<'_, AppState>) -> Result<crate::model::Stats> {
    state.ctx.read().await.store.stats().await
}
