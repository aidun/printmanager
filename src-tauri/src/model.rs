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
    /// Kundennummer (fortlaufend, angezeigt als K-0001)
    pub number: i64,
    pub name: String,
    pub email: String,
    pub phone: String,
    pub street: String,
    pub house_no: String,
    pub zip: String,
    pub city: String,
    /// abweichende Rechnungsadresse (0/1)
    pub billing_differs: i64,
    pub billing_name: String,
    pub billing_street: String,
    pub billing_house_no: String,
    pub billing_zip: String,
    pub billing_city: String,
    pub note: String,
    pub created_at: String,
    /// Anzahl Aufträge des Kunden
    pub order_count: i64,
    /// davon offen (nicht übergeben/abgelehnt, nicht archiviert)
    pub open_count: i64,
    /// Anlagezeitpunkt des neuesten Auftrags
    pub last_order_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonInput {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub street: String,
    #[serde(default)]
    pub house_no: String,
    #[serde(default)]
    pub zip: String,
    #[serde(default)]
    pub city: String,
    /// Rechnungsadresse weicht ab; sonst werden die Rechnungsfelder geleert
    #[serde(default)]
    pub billing_differs: bool,
    #[serde(default)]
    pub billing_name: String,
    #[serde(default)]
    pub billing_street: String,
    #[serde(default)]
    pub billing_house_no: String,
    #[serde(default)]
    pub billing_zip: String,
    #[serde(default)]
    pub billing_city: String,
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
    /// Kunde (Pflicht: entweder person_id oder person_name)
    pub person_id: Option<String>,
    /// Kunde über den Namen finden oder neu anlegen, wenn kein person_id angegeben ist
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
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

#[derive(Debug, Clone, Default, Serialize, sqlx::FromRow)]
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
    /// RFID-Kennung (Bambu-Spule); bei RFID pflegt das AMS den Rest
    pub rfid: String,
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
    pub rfid: String,
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
    /// Slot im Projekt (1-basiert), falls bekannt; trennt gleiche Filamente in verschiedenen Slots
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<i64>,
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
    pub status: String,
    /// Dateianhänge und Dateien erkannter Drucke
    pub files: Vec<String>,
    /// Anlagezeitpunkt des neuesten erkannten Drucks
    pub last_run_at: Option<String>,
    /// Anlagezeitpunkt des Auftrags (ältere Drucke werden ihm nicht automatisch zugeordnet)
    pub created_at: String,
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
    /// alle Drucke aus dem Druckverlauf des Kontos (leer ohne Anmeldung)
    pub history: Vec<HistoryEntry>,
}

/// Ein Druck aus dem Druckverlauf des Bambu-Kontos
#[derive(Debug, Clone, Default, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub design_title: String,
    pub design_id: Option<String>,
    pub device_name: String,
    pub device_model: String,
    pub plate_index: Option<i64>,
    /// 2 fertig, 3 abgebrochen/Fehler, sonst laufend/unbekannt
    pub status: i64,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub planned_seconds: Option<i64>,
    pub weight_grams: Option<f64>,
    /// JSON-Liste von RunFilament (Slicer-Werte je AMS-Slot)
    pub filaments: String,
    pub cover_url: Option<String>,
    pub order_id: Option<String>,
    pub matched_by: Option<String>,
    /// zuletzt live gesehener Fortschritt in % (nur bei Drucken, die die App beobachtet hat)
    pub progress_at_end: Option<i64>,
    /// Anteil des geplanten Verbrauchs, der tatsächlich gedruckt wurde (1.0 bei fertigen Drucken)
    pub used_factor: Option<f64>,
    /// fertig | fortschritt | zeit | geplant
    pub used_method: Option<String>,
    /// aus dem Auftrag (nur in Listen gefüllt)
    pub order_number: Option<i64>,
    pub order_title: Option<String>,
    pub person_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySyncResult {
    pub fetched: i64,
    pub added: i64,
    pub matched: i64,
    pub total: i64,
}

/// Druckprofil und Schätzung eines Teils (Kalkulation vor dem eigenen Slicen)
#[derive(Debug, Clone, Default, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemEstimate {
    pub item_id: String,
    pub source: String,
    pub printer: String,
    pub nozzle: String,
    pub process: String,
    pub filament: String,
    pub bed_type: String,
    /// slicer | makerworld (leer = keine Schätzung)
    pub basis: String,
    pub seconds: Option<i64>,
    pub filaments: String,
    pub mw_instance: String,
    pub error: String,
    pub updated_at: Option<String>,
    /// Filament-Slots als JSON (`Vec<SetupSlot>`)
    pub slots: String,
    /// Objekte des Modells mit Slot als JSON (`Vec<SetupObject>`)
    pub objects: String,
    /// '' | entwurf | bereit | uebergeben
    pub setup_state: String,
    pub project_file: String,
}

/// Filament-Slot der Druckvorbereitung (1-basiert wie in Bambu Studio)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SetupSlot {
    pub slot: i64,
    /// Datei des Filament-Presets in der Profilquelle
    pub preset: String,
    pub material: String,
    /// #RRGGBB
    pub color: String,
    pub spool_id: Option<String>,
}

/// Objekt im Modell und sein Slot (Reihenfolge wie `--load-filament-ids`)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SetupObject {
    pub index: i64,
    pub name: String,
    pub slot: i64,
}

/// Verknüpfung eines lokalen Datensatzes mit seinem Gegenstück bei einer Anbindung
#[derive(Debug, Clone, PartialEq, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLink {
    pub id: String,
    /// ID der Anbindung, z.B. "google-contacts"
    pub connector: String,
    /// Art des lokalen Datensatzes: person | spool | catalog | print
    pub entity: String,
    pub external_id: String,
    pub local_id: String,
    pub etag: String,
    pub remote_updated_at: Option<String>,
    /// Hash der zugeordneten Felder beim letzten Abgleich (Konfliktprüfung)
    pub synced_hash: String,
    pub local_hash: String,
    pub last_synced_at: Option<String>,
    /// letzter gemeinsamer Stand der Quelle (JSON der zugeordneten Felder)
    pub remote_snapshot: String,
    /// letzter gemeinsamer Stand in der App (JSON der zugeordneten Felder)
    pub local_snapshot: String,
    /// ok | konflikt | geloescht
    pub state: String,
    /// Felder im Konflikt, durch Komma getrennt
    pub conflict: String,
}

/// Neue oder geänderte Verknüpfung (ohne ID)
#[derive(Debug, Clone, Default)]
pub struct LinkInput {
    pub connector: String,
    pub entity: String,
    pub external_id: String,
    pub local_id: String,
    pub etag: String,
    pub remote_updated_at: Option<String>,
    pub synced_hash: String,
    pub local_hash: String,
    pub remote_snapshot: String,
    pub local_snapshot: String,
    pub state: String,
    pub conflict: String,
}

/// Ein Schreibvorgang bei einer Anbindung (Protokoll)
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorWrite {
    pub id: String,
    pub created_at: String,
    /// Anbindung, z.B. "bambu-cloud"
    pub connector: String,
    /// Art des lokalen Datensatzes, z.B. "spool"
    pub entity: String,
    pub local_id: String,
    pub label: String,
    pub action: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub status: String,
    pub message: String,
}
