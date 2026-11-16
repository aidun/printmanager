use serde::{Deserialize, Serialize};

/// Ablauf eines Auftrags. Die Reihenfolge ist die der Spalten im Board.
pub const STATUSES: &[&str] = &[
    "eingang",
    "klaeren",
    "vorbereiten",
    "in_studio",
    "druckt",
    "nacharbeit",
    "fertig",
    "uebergeben",
    "abgelehnt",
    "pausiert",
];

pub fn valid_status(s: &str) -> bool {
    STATUSES.contains(&s)
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub name: String,
    pub contact: String,
    pub note: String,
    pub created_at: String,
    /// Anzahl Aufträge der Person (nur in Listen gefüllt)
    pub order_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub contact: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OrderSummary {
    pub id: String,
    pub number: i64,
    pub title: String,
    pub status: String,
    pub priority: i64,
    pub due_date: Option<String>,
    pub position: i64,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub item_count: i64,
    pub attachment_count: i64,
    /// relativer Pfad des ersten Bildes (für die Vorschau auf der Karte)
    pub cover_path: Option<String>,
    /// neueste Druckdatei (3MF/STL/STEP) zum direkten Öffnen in Bambu Studio
    pub print_file_id: Option<String>,
    /// Drucke ohne Ergebnis bzw. ohne abgezogenen Verbrauch
    pub runs_open: i64,
    pub runs_undeducted: i64,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub id: String,
    pub number: i64,
    pub person_id: Option<String>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i64,
    pub due_date: Option<String>,
    pub position: i64,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInput {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub person_id: Option<String>,
    /// Neue Person anlegen, wenn kein person_id angegeben ist
    pub person_name: Option<String>,
    #[serde(default)]
    pub priority: i64,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderUpdate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub person_id: Option<String>,
    pub priority: i64,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(default)]
    pub id: String,
    pub order_id: String,
    #[serde(default)]
    pub position: i64,
    pub title: String,
    pub quantity: i64,
    #[serde(default)]
    pub color_wish: String,
    #[serde(default)]
    pub note: String,
    pub catalog_id: Option<String>,
    #[serde(default)]
    pub done: i64,
    /// Druckdatei des Teils (Anhang im Auftragsordner)
    #[serde(default)]
    pub file_attachment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub order_id: String,
    pub kind: String,
    pub name: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub body: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub order_id: String,
    pub kind: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDetail {
    pub order: Order,
    pub person: Option<Person>,
    pub items: Vec<Item>,
    pub attachments: Vec<Attachment>,
    pub events: Vec<Event>,
    pub runs: Vec<PrintRun>,
    /// absoluter Pfad des Auftragsordners
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub file_name: String,
    pub thumb_name: Option<String>,
    pub source_url: Option<String>,
    pub designer: String,
    pub license: String,
    pub plates: i64,
    pub print_seconds: Option<i64>,
    pub weight_grams: Option<i64>,
    /// JSON-Liste [{"type","color","grams"}]
    pub filaments: String,
    pub tags: String,
    pub hidden: i64,
    pub created_at: String,
    pub updated_at: String,
    /// in wie vielen Teilen von Aufträgen verwendet
    pub usage_count: i64,
    /// lokal | makerworld
    pub origin: String,
    pub remote_id: Option<String>,
    pub cover_url: Option<String>,
    pub remote_missing: i64,
    /// MakerWorld-Sammlungen, in denen das Projekt steckt (IDs)
    #[sqlx(skip)]
    pub collection_ids: Vec<String>,
}

/// Eine MakerWorld-Sammlung
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MwCollection {
    pub id: String,
    pub title: String,
    pub design_cnt: i64,
    pub is_default: i64,
    pub enabled: i64,
    pub synced_at: Option<String>,
}

/// Ein Modell aus einer MakerWorld-Sammlung (Listendaten, ohne Datei)
#[derive(Debug, Clone, Default)]
pub struct MwDesign {
    pub id: String,
    pub title: String,
    pub designer: String,
    pub cover: Option<String>,
    pub slug: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MwSyncResult {
    pub collections: i64,
    pub designs: i64,
    pub added: i64,
    pub missing: i64,
}

#[derive(Debug, Clone, Default)]
pub struct NewCatalogEntry {
    pub title: String,
    pub description: String,
    pub file_name: String,
    pub thumb_name: Option<String>,
    pub source_url: Option<String>,
    pub designer: String,
    pub license: String,
    pub plates: i64,
    pub print_seconds: Option<i64>,
    pub weight_grams: Option<i64>,
    pub filaments: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogUpdate {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: String,
    pub hidden: i64,
}

// ---------- Filament ----------

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Spool {
    pub id: String,
    pub brand: String,
    pub series: String,
    pub material: String,
    pub color: String,
    pub color_name: String,
    pub initial_grams: f64,
    pub studio_grams: f64,
    pub remaining_grams: f64,
    pub studio_status: String,
    pub hidden: i64,
    pub missing: i64,
    pub note: String,
    pub updated_at: String,
    /// datei | konto
    pub source: String,
    pub source_updated_at: Option<String>,
    pub in_printer: i64,
    pub printer_name: String,
    pub ams_slot: String,
    pub depleted: i64,
}

/// Eine Spule aus Bambu Studios `spools.json` oder aus dem Bambu-Konto.
#[derive(Debug, Clone, Default)]
pub struct StudioSpool {
    pub id: String,
    pub brand: String,
    pub series: String,
    pub material: String,
    pub color: String,
    pub color_name: String,
    pub initial_grams: f64,
    pub net_grams: f64,
    pub status: String,
    /// datei | konto
    pub source: String,
    pub source_updated_at: Option<String>,
    pub in_printer: bool,
    pub printer_name: String,
    pub ams_slot: String,
    pub depleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpoolUpdate {
    pub id: String,
    pub remaining_grams: f64,
    pub hidden: i64,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    /// datei | konto
    pub source: String,
    pub path: String,
    pub found: bool,
    pub added: i64,
    pub updated: i64,
    pub missing: i64,
}

/// Filament eines Drucks (JSON in `print_runs.filaments`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunFilament {
    #[serde(rename = "type")]
    pub kind: String,
    pub color: String,
    pub grams: f64,
    /// Spule, von der abgezogen wurde
    #[serde(default)]
    pub spool_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PrintRun {
    pub id: String,
    pub order_id: String,
    pub item_id: Option<String>,
    pub attachment_id: Option<String>,
    pub file_name: String,
    pub plates: i64,
    pub print_seconds: Option<i64>,
    pub weight_grams: Option<f64>,
    /// JSON-Liste von [`RunFilament`]
    pub filaments: String,
    pub result: String,
    pub deducted: i64,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
    /// Rückmeldung des Druckers über das Bambu-Konto
    pub printer_name: String,
    /// '' | ok | fehldruck
    pub printer_status: String,
    pub printer_finished_at: Option<String>,
    pub actual_seconds: Option<i64>,
    /// JSON-Liste von [`RunFilament`] mit den echten Gramm laut Drucker
    pub actual_filaments: String,
}

/// Offener Auftrag mit seinen Druckdateien, um Druckaufträge vom Drucker zuzuordnen.
#[derive(Debug, Clone)]
pub struct OrderFiles {
    pub order_id: String,
    pub number: i64,
    pub title: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NewRun {
    pub order_id: String,
    pub item_id: Option<String>,
    pub attachment_id: Option<String>,
    pub file_name: String,
    pub plates: i64,
    pub print_seconds: Option<i64>,
    pub weight_grams: Option<f64>,
    pub filaments: Vec<RunFilament>,
    pub signature: String,
}

/// Zuordnung beim Abziehen: Filament `index` des Drucks → Spule, mit ggf. korrigierten Gramm.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deduction {
    pub index: usize,
    pub spool_id: Option<String>,
    pub grams: f64,
}

pub const RUN_RESULTS: &[&str] = &["offen", "ok", "fehldruck"];

/// Ein Druck für die Statistik
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StatRun {
    pub created_at: String,
    pub result: String,
    pub print_seconds: Option<i64>,
    /// JSON-Liste von `RunFilament`
    pub filaments: String,
    pub order_number: i64,
    pub person_name: Option<String>,
}

/// Eine Übergabe (Statuswechsel auf „Übergeben“) für die Statistik
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct StatHandover {
    pub created_at: String,
    pub order_number: i64,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub runs: Vec<StatRun>,
    pub handovers: Vec<StatHandover>,
}
