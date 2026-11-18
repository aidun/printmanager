mod bambu;
mod commands;
mod connectors;
mod error;
mod filament;
mod files;
mod linkimport;
mod matching;
mod model;
mod pricing;
mod settings;
mod slicers;
mod store;
mod threemf;
mod usage;
mod watch;

use connectors::registry::Registry;
use connectors::secrets::{KeyringBackend, SecretBackend};
use std::sync::Arc;

use commands::*;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings_file = app.path().app_config_dir()?.join("settings.json");
            let default_data_dir = app.path().app_data_dir()?;
            let settings = settings::Settings::load(&settings_file);
            let ctx = tauri::async_runtime::block_on(async {
                match open_ctx(&handle, settings.clone(), &default_data_dir).await {
                    Ok(ctx) => Ok(ctx),
                    // Eigener Datenordner/Datenbank nicht erreichbar: mit dem Standardordner starten,
                    // damit die App sich trotzdem öffnet und die Einstellungen änderbar bleiben.
                    Err(e) => {
                        eprintln!("Datenordner nicht nutzbar ({e}), nutze Standardordner");
                        open_ctx(&handle, settings::Settings::default(), &default_data_dir).await
                    }
                }
            })?;
            // Anbindungen: Einstellungen aus 0.3 übernehmen, Registry aufbauen, gemeinsamer Takt
            let config_dir = app.path().app_config_dir()?;
            if let Err(e) = bambu::migrate_config(&config_dir) {
                eprintln!("Bambu-Einstellungen nicht übernommen: {e}");
            }
            let secrets: Arc<dyn SecretBackend> = Arc::new(KeyringBackend);
            let registry = Arc::new(Registry::new(config_dir, secrets.clone(), vec![
                    Arc::new(bambu::BambuConnector::new(app.handle().clone())),
                    Arc::new(connectors::PublicConnector(&linkimport::WEB)),
                    Arc::new(connectors::PublicConnector(&linkimport::PRINTABLES)),
                    Arc::new(slicers::bambu_studio::BambuStudio),
                ])?);
            if let Err(e) = bambu::migrate_token(secrets.as_ref(), &registry.get(bambu::ID)?.ctx.secrets) {
                eprintln!("Bambu-Token nicht übernommen: {e}");
            }
            // Filament-Datei des Slicers (heute Bambu Studio)
            let system_data_dir = app.path().data_dir()?;
            let default_spools_file = tauri::async_runtime::block_on(registry.slicer())
                .and_then(|c| c.slicer().and_then(|s| s.default_spools_file(&system_data_dir)))
                .unwrap_or_default();
            let state = AppState {
                ctx: tokio::sync::RwLock::new(ctx),
                settings_file,
                default_data_dir,
                default_spools_file,
                watcher: std::sync::Mutex::new(None),
            };
            let (orders, spools) = {
                let ctx = state.ctx.try_read().expect("frisch angelegt");
                (ctx.data_dir.join("orders"), state.spools_file(&ctx))
            };
            app.manage(state);
            app.manage(bambu::Bambu::new(registry.clone())?);
            app.manage(registry);
            watch::restart(app.handle(), orders, spools);
            connectors::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            settings_save,
            people_list,
            person_save,
            person_delete,
            orders_list,
            order_get,
            order_create,
            order_update,
            order_set_status,
            order_delete,
            note_add,
            item_save,
            item_delete,
            attachment_add_paths,
            attachment_add_bytes,
            attachment_add_link,
            attachment_add_text,
            attachment_delete,
            attachment_open,
            attachment_open_in_studio,
            order_reveal,
            open_url,
            catalog_list,
            catalog_import_paths,
            catalog_update,
            catalog_delete,
            catalog_open_in_studio,
            catalog_add_to_order,
            catalog_import_link,
            link_preview,
            link_import_to_order,
            spools_list,
            spool_update,
            run_set_result,
            run_deduct,
            run_undo,
            run_delete,
            order_archive,
            orders_archive_closed,
            stats,
            bambu::bambu_status,
            connectors::commands::connectors_list,
            connectors::commands::connector_connect,
            connectors::commands::connector_disconnect,
            connectors::commands::connector_set_enabled,
            connectors::commands::connector_set_cap,
            connectors::commands::connector_writes,
            connectors::commands::connector_write_undo,
            bambu::filament_sync,
            bambu::printers::printers_list,
            bambu::printers::printers_refresh,
            bambu::printers::printer_assign,
            bambu::history::history_list,
            bambu::history::history_sync,
            bambu::history::history_assign,
            bambu::filament_write::spool_push_rest,
            bambu::filament_write::spool_stock_add,
            bambu::filament_write::spool_assign_slot,
            bambu::filament_write::spool_create,
            bambu::filament_write::spool_stock_remove,
            pricing::pricing_get,
            pricing::pricing_save,
            bambu::makerworld::mw_collections,
            bambu::makerworld::mw_collection_toggle,
            bambu::makerworld::mw_sync,
            bambu::makerworld::catalog_fetch_file,
            order_scan,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der App");
}
