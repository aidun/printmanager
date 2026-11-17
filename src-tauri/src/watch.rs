//! Rückweg aus Bambu Studio: Die App beobachtet die Auftragsordner und `spools.json`.
//! Speicherst du in Bambu Studio eine gesliced 3MF im Auftragsordner, legt die App einen Druck an
//! (Druckzeit, Gramm pro Filament). Neue 3MF-Dateien im Ordner werden als Anhang übernommen.

use crate::commands::{add_file_attachment, AppState, Ctx};
use crate::error::Result;
use crate::model::{NewRun, RunFilament};
use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Ereignis an die Oberfläche: Daten haben sich außerhalb der App geändert.
pub const EVENT: &str = "pm:external";

#[derive(Clone, Serialize)]
pub struct Changed {
    pub messages: Vec<String>,
}

/// Toleranz: Dateien, die höchstens so lange nach dem Anhängen geändert wurden, gelten als unverändert.
const SETTLE_SECS: i64 = 10;

/// Beobachtung (neu) starten, z.B. nach dem Wechsel des Datenordners. Prüft danach einmal alles.
pub fn restart(app: &AppHandle, orders: PathBuf, spools: PathBuf) {
    let state = app.state::<AppState>();
    let mut slot = state.watcher.lock().unwrap();
    *slot = None; // alten Beobachter beenden
    match start(app.clone(), &orders, &spools) {
        Ok(w) => *slot = Some(w),
        Err(e) => eprintln!("Ordner können nicht beobachtet werden: {e}"),
    }
    drop(slot);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let ctx = state.ctx.read().await;
        let mut messages = Vec::new();
        let tagged = crate::commands::tag_open_model_files(&ctx).await;
        if tagged > 0 {
            eprintln!("{tagged} Modelldateien mit Auftragsnummer versehen");
        }
        let from_file = !app.state::<crate::bambu::Bambu>().filament_active().await;
        if !from_file {
            // Filament kommt aus dem Bambu-Konto (eigener Takt in bambu.rs)
        } else if let Ok(r) = sync_spools(&ctx, &state.spools_file(&ctx)).await {
            if r.added > 0 {
                messages.push(format!("{} Spulen aus Bambu Studio übernommen", r.added));
            }
        }
        for f in all_order_files(&ctx.data_dir.join("orders")) {
            match handle_file(&ctx, &f).await {
                Ok(Some(m)) => messages.push(m),
                Ok(None) => {}
                Err(e) => eprintln!("{}: {e}", f.display()),
            }
        }
        if !messages.is_empty() {
            let _ = app.emit(EVENT, Changed { messages });
        }
    });
}

fn start(app: AppHandle, orders: &Path, spools: &Path) -> notify::Result<notify::RecommendedWatcher> {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if ev.kind.is_create() || ev.kind.is_modify() {
                for p in ev.paths {
                    let _ = tx.send(p);
                }
            }
        }
    })?;
    std::fs::create_dir_all(orders).ok();
    w.watch(orders, RecursiveMode::Recursive)?;
    if let Some(dir) = spools.parent().filter(|d| d.is_dir()) {
        let _ = w.watch(dir, RecursiveMode::NonRecursive);
    }
    let spools = spools.to_path_buf();
    std::thread::spawn(move || loop {
        // Auf Ruhe warten: Bambu Studio schreibt große Dateien in mehreren Schritten.
        let Ok(first) = rx.recv() else { return };
        let mut paths = BTreeSet::from([first]);
        loop {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(p) => {
                    paths.insert(p);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        tauri::async_runtime::block_on(process(&app, &spools, paths));
    });
    Ok(w)
}

async fn process(app: &AppHandle, spools: &Path, paths: BTreeSet<PathBuf>) {
    let state = app.state::<AppState>();
    let ctx = state.ctx.read().await;
    let mut messages = Vec::new();
    let mut changed = false;
    for p in &paths {
        if p.file_name() == spools.file_name() && p.parent() == spools.parent() {
            if app.state::<crate::bambu::Bambu>().filament_active().await {
                continue; // Filament kommt aus dem Bambu-Konto
            }
            if let Ok(r) = sync_spools(&ctx, spools).await {
                changed = true;
                if r.added + r.updated > 0 {
                    messages.push("Spulen aus Bambu Studio aktualisiert".into());
                }
            }
            continue;
        }
        match handle_file(&ctx, p).await {
            Ok(Some(m)) => messages.push(m),
            Ok(None) => {}
            Err(e) => eprintln!("{}: {e}", p.display()),
        }
    }
    if changed || !messages.is_empty() {
        let _ = app.emit(EVENT, Changed { messages });
    }
}

pub async fn sync_spools(ctx: &Ctx, file: &Path) -> Result<crate::model::SyncResult> {
    let mut r = match crate::filament::read(file)? {
        Some(list) => ctx.store.sync_spools(&list).await?,
        None => Default::default(),
    };
    r.path = file.display().to_string();
    Ok(r)
}

fn is_3mf(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    !name.starts_with('.') && !name.starts_with('~') && name.to_lowercase().ends_with(".3mf")
}

/// Alle 3MF-Dateien direkt in `orders/<nr>/`.
fn all_order_files(orders: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in std::fs::read_dir(orders).into_iter().flatten().flatten() {
        if d.path().is_dir() {
            for f in std::fs::read_dir(d.path()).into_iter().flatten().flatten() {
                if f.path().is_file() && is_3mf(&f.path()) {
                    out.push(f.path());
                }
            }
        }
    }
    out
}

/// Alle 3MF-Dateien eines Auftragsordners prüfen (Knopf „Ordner prüfen“).
pub async fn scan_order(ctx: &Ctx, dir: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for f in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if f.path().is_file() && is_3mf(&f.path()) {
            if let Some(m) = handle_file(ctx, &f.path()).await? {
                out.push(m);
            }
        }
    }
    Ok(out)
}

/// Eine Datei im Auftragsordner auswerten. Rückgabe: Meldung, falls etwas angelegt wurde.
pub async fn handle_file(ctx: &Ctx, path: &Path) -> Result<Option<String>> {
    let orders = ctx.data_dir.join("orders");
    let Ok(rel) = path.strip_prefix(&orders) else { return Ok(None) };
    let parts: Vec<&str> = rel.iter().filter_map(|c| c.to_str()).collect();
    let [nr, name] = parts[..] else { return Ok(None) };
    if !is_3mf(path) || !path.is_file() {
        return Ok(None);
    }
    let Ok(number) = nr.parse::<i64>() else { return Ok(None) };
    let Some(order) = ctx.store.order_by_number(number).await? else { return Ok(None) };

    let mut messages = Vec::new();
    let attachment_id = match ctx.store.attachment_by_path(&order.id, name).await? {
        Some(a) => {
            // Noch so, wie die App die Datei abgelegt hat? Dann ist es keine Arbeit aus Bambu Studio.
            let modified: chrono::DateTime<chrono::Utc> = std::fs::metadata(path)?.modified()?.into();
            let added = chrono::DateTime::parse_from_rfc3339(&a.created_at).map(|d| d.to_utc()).unwrap_or(modified);
            if modified <= added + chrono::Duration::seconds(SETTLE_SECS) {
                return Ok(None);
            }
            a.id
        }
        None => {
            let id = add_file_attachment(ctx, &order.id, name).await?;
            messages.push(format!("#{}: neue Datei {name}", order.number));
            id
        }
    };

    // Unvollständig geschriebene Dateien lassen sich nicht lesen; das nächste Ereignis kommt dann noch.
    let Ok(info) = crate::threemf::analyze(path) else { return Ok(messages.pop()) };
    if !info.sliced {
        return Ok(messages.pop());
    }
    let filaments: Vec<RunFilament> = info
        .filaments
        .iter()
        .filter(|f| f.grams.unwrap_or(0.0) > 0.0)
        .map(|f| RunFilament {
            kind: f.kind.clone(),
            color: f.color.clone(),
            grams: (f.grams.unwrap_or(0.0) * 100.0).round() / 100.0,
            spool_id: None,
        })
        .collect();
    let signature = format!(
        "{}|{}|{}",
        info.print_seconds.unwrap_or(0),
        info.plates,
        filaments.iter().map(|f| format!("{}{}{:.2}", f.kind, f.color, f.grams)).collect::<Vec<_>>().join(",")
    );
    if ctx.store.last_run_signature(&order.id, name).await?.as_deref() == Some(signature.as_str()) {
        return Ok(messages.pop());
    }
    let total: f64 = filaments.iter().map(|f| f.grams).sum();
    ctx.store
        .insert_run(NewRun {
            order_id: order.id.clone(),
            item_id: None,
            attachment_id: Some(attachment_id),
            file_name: name.to_string(),
            plates: info.plates,
            print_seconds: info.print_seconds,
            weight_grams: info.weight_grams,
            filaments,
            signature,
        })
        .await?;
    if ["eingang", "klaeren", "vorbereiten", "in_studio"].contains(&order.status.as_str()) {
        ctx.store.set_status(&order.id, "druckt", Some("Gesliced Datei aus Bambu Studio erkannt")).await?;
    }
    Ok(Some(format!("#{} {}: Druck erkannt ({:.0} g) – Verbrauch im Auftrag abziehen", order.number, order.title, total)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;

    /// Gesliced Datei aus `tests/fixtures/` (nur lokal, nicht im Repo) in einen Auftragsordner legen.
    #[tokio::test]
    async fn gesliced_datei_erkennen() {
        let sliced = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/figuren.gcode.3mf");
        if !sliced.exists() {
            return;
        }
        let dir = tempfile::Builder::new().prefix("pm watch ").tempdir().unwrap();
        let store = Store::connect(&Store::sqlite_url(&dir.path().join("t.db"))).await.unwrap();
        let ctx = Ctx { store, data_dir: dir.path().into(), settings: Default::default() };
        let o = ctx
            .store
            .create_order(crate::model::OrderInput {
                title: "Figuren".into(),
                description: String::new(),
                person_id: None,
                person_name: Some("Testkunde".into()),
                priority: 0,
                due_date: None,
            })
            .await
            .unwrap();
        let folder = crate::files::order_dir(dir.path(), o.number);
        std::fs::create_dir_all(&folder).unwrap();
        let target = folder.join("figuren.gcode.3mf");
        std::fs::copy(&sliced, &target).unwrap();

        let msg = handle_file(&ctx, &target).await.unwrap();
        assert!(msg.unwrap().contains("Druck erkannt"));
        let d = ctx.store.order_detail(&o.id, String::new()).await.unwrap();
        assert_eq!(d.runs.len(), 1);
        assert_eq!(d.attachments.len(), 1, "neue Datei als Anhang übernommen");
        assert_eq!(d.order.status, "druckt");
        let fils: Vec<RunFilament> = serde_json::from_str(&d.runs[0].filaments).unwrap();
        assert!(fils[0].grams > 0.0);

        // Erneutes Speichern ohne neues Slicen erzeugt keinen zweiten Druck
        assert!(handle_file(&ctx, &target).await.unwrap().is_none());
        // Dateien außerhalb von orders/<nr>/ werden ignoriert
        assert!(handle_file(&ctx, &dir.path().join("t.db")).await.unwrap().is_none());
    }

    /// Beim Start bekommen Modelldateien offener Aufträge die Auftragskennung; Bilder bleiben.
    #[tokio::test]
    async fn modelldateien_beim_start_kennzeichnen() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::connect(&Store::sqlite_url(&dir.path().join("t.db"))).await.unwrap();
        let ctx = Ctx { store, data_dir: dir.path().into(), settings: Default::default() };
        let o = ctx
            .store
            .create_order(crate::model::OrderInput {
                title: "Benchy".into(),
                description: String::new(),
                person_id: None,
                person_name: Some("Testkunde".into()),
                priority: 0,
                due_date: None,
            })
            .await
            .unwrap();
        let folder = crate::files::order_dir(dir.path(), o.number);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("3dbenchy.stl"), b"solid x").unwrap();
        std::fs::write(folder.join("bild.jpg"), b"jpg").unwrap();
        add_file_attachment(&ctx, &o.id, "3dbenchy.stl").await.unwrap();
        add_file_attachment(&ctx, &o.id, "bild.jpg").await.unwrap();

        assert_eq!(crate::commands::tag_open_model_files(&ctx).await, 1);
        assert!(folder.join("A0001 3dbenchy.stl").is_file() && !folder.join("3dbenchy.stl").exists());
        assert!(folder.join("bild.jpg").is_file(), "Bilder bleiben");
        let d = ctx.store.order_detail(&o.id, String::new()).await.unwrap();
        assert!(d.attachments.iter().any(|a| a.path.as_deref() == Some("A0001 3dbenchy.stl")));
        // zweiter Lauf ändert nichts mehr
        assert_eq!(crate::commands::tag_open_model_files(&ctx).await, 0);
        // der Beobachter legt für die umbenannte Datei keinen neuen Anhang an
        assert!(handle_file(&ctx, &folder.join("A0001 3dbenchy.stl")).await.unwrap().is_none());
        assert_eq!(ctx.store.order_detail(&o.id, String::new()).await.unwrap().attachments.len(), 2);
    }
}
