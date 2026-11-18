//! Tauri-Befehle für die Einstellungsseite „Anbindungen“ (Stufe A6): Liste, Verbinden, Trennen,
//! Schalter und das gemeinsame Schreibprotokoll.

use super::registry::{ConnectorInfo, Registry};
use super::Cap;
use crate::commands::AppState;
use crate::error::{AppError, Result};
use crate::model::ConnectorWrite;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn connectors_list(registry: State<'_, Arc<Registry>>) -> Result<Vec<ConnectorInfo>> {
    Ok(registry.list().await)
}

/// Anmelden bzw. Freigabe holen. Bei Bambu öffnet sich das Anmeldefenster; der Rest läuft im Hintergrund.
#[tauri::command]
pub async fn connector_connect(registry: State<'_, Arc<Registry>>, id: String) -> Result<()> {
    registry.connect(&id).await
}

/// Trennen: Geheimnisse löschen und ausschalten. Lokale Daten und Verknüpfungen bleiben.
#[tauri::command]
pub async fn connector_disconnect(registry: State<'_, Arc<Registry>>, id: String) -> Result<()> {
    registry.disconnect(&id).await
}

#[tauri::command]
pub async fn connector_set_enabled(registry: State<'_, Arc<Registry>>, id: String, enabled: bool) -> Result<()> {
    registry.set_enabled(&id, enabled).await
}

#[tauri::command]
pub async fn connector_set_cap(registry: State<'_, Arc<Registry>>, id: String, cap: Cap, on: bool) -> Result<()> {
    registry.set_cap(&id, cap, on).await
}

/// Die letzten Schreibvorgänge aller Anbindungen
#[tauri::command]
pub async fn connector_writes(state: State<'_, AppState>) -> Result<Vec<ConnectorWrite>> {
    state.ctx.read().await.store.list_connector_writes(50).await
}

/// Einen Schreibvorgang zurücknehmen; die Anbindung, die ihn ausgeführt hat, entscheidet wie.
#[tauri::command]
pub async fn connector_write_undo(app: AppHandle, state: State<'_, AppState>, write_id: String) -> Result<crate::bambu::filament_write::WriteResult> {
    let w = state.ctx.read().await.store.get_connector_write(&write_id).await?;
    match w.connector.as_str() {
        crate::bambu::ID => crate::bambu::filament_write::undo(&app, &w).await,
        other => Err(AppError::Invalid(format!("Für „{other}“ gibt es kein Rückgängig."))),
    }
}
