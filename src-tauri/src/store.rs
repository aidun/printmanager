//! Repository-Schicht. Alle Abfragen nutzen portables SQL mit `$n`-Platzhaltern,
//! damit dieselbe Schicht später auch gegen PostgreSQL läuft (sqlx `Any`-Treiber).

use crate::error::{AppError, Result};
use crate::model::*;
use sqlx::any::{install_default_drivers, AnyPoolOptions};
use sqlx::{AnyPool, Row};

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone)]
pub struct Store {
    pool: AnyPool,
}

const ORDER_SUMMARY_SQL: &str = "
    SELECT o.id, o.number, o.title, o.status, o.priority, o.due_date, o.position,
           o.person_id, p.name AS person_name,
           (SELECT COUNT(*) FROM order_items i WHERE i.order_id = o.id) AS item_count,
           (SELECT COUNT(*) FROM attachments a WHERE a.order_id = o.id) AS attachment_count,
           (SELECT a.path FROM attachments a WHERE a.order_id = o.id AND a.kind = 'image'
              ORDER BY a.created_at LIMIT 1) AS cover_path,
           (SELECT a.id FROM attachments a WHERE a.order_id = o.id AND a.kind = 'file'
              AND (LOWER(a.name) LIKE '%.3mf' OR LOWER(a.name) LIKE '%.stl' OR LOWER(a.name) LIKE '%.step'
                   OR LOWER(a.name) LIKE '%.stp' OR LOWER(a.name) LIKE '%.obj')
              ORDER BY a.created_at DESC LIMIT 1) AS print_file_id,
           (SELECT COUNT(*) FROM print_runs r WHERE r.order_id = o.id AND r.result = 'offen') AS runs_open,
           (SELECT COUNT(*) FROM print_runs r WHERE r.order_id = o.id AND r.deducted = 0) AS runs_undeducted,
           o.archived_at, o.created_at, o.updated_at
    FROM orders o LEFT JOIN people p ON p.id = o.person_id";

impl Store {
    /// `url`: z.B. `sqlite:///pfad/printmanager.db?mode=rwc` oder `postgres://…`
    pub async fn connect(url: &str) -> Result<Self> {
        install_default_drivers();
        let pool = AnyPoolOptions::new().max_connections(4).connect(url).await?;
        if url.starts_with("sqlite:") {
            sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;
            sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
        }
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn sqlite_url(path: &std::path::Path) -> String {
        format!("sqlite://{}?mode=rwc", path.display())
    }

    // ---------- Personen ----------

    pub async fn list_people(&self) -> Result<Vec<Person>> {
        Ok(sqlx::query_as::<_, Person>(
            "SELECT p.id, p.name, p.contact, p.note, p.created_at,
                    (SELECT COUNT(*) FROM orders o WHERE o.person_id = p.id) AS order_count
             FROM people p ORDER BY LOWER(p.name)",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_person(&self, id: &str) -> Result<Option<Person>> {
        Ok(sqlx::query_as::<_, Person>(
            "SELECT p.id, p.name, p.contact, p.note, p.created_at,
                    (SELECT COUNT(*) FROM orders o WHERE o.person_id = p.id) AS order_count
             FROM people p WHERE p.id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn save_person(&self, p: PersonInput) -> Result<String> {
        let name = p.name.trim();
        if name.is_empty() {
            return Err(AppError::Invalid("Der Name darf nicht leer sein.".into()));
        }
        match p.id {
            Some(id) => {
                let r = sqlx::query("UPDATE people SET name = $1, contact = $2, note = $3 WHERE id = $4")
                    .bind(name)
                    .bind(p.contact.trim())
                    .bind(p.note.trim())
                    .bind(&id)
                    .execute(&self.pool)
                    .await?;
                if r.rows_affected() == 0 {
                    return Err(AppError::NotFound("Person".into()));
                }
                Ok(id)
            }
            None => {
                let id = new_id();
                sqlx::query("INSERT INTO people (id, name, contact, note, created_at) VALUES ($1, $2, $3, $4, $5)")
                    .bind(&id)
                    .bind(name)
                    .bind(p.contact.trim())
                    .bind(p.note.trim())
                    .bind(now())
                    .execute(&self.pool)
                    .await?;
                Ok(id)
            }
        }
    }

    pub async fn delete_person(&self, id: &str) -> Result<()> {
        let n: i64 = sqlx::query("SELECT COUNT(*) AS n FROM orders WHERE person_id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await?
            .try_get("n")?;
        if n > 0 {
            return Err(AppError::Invalid(format!(
                "Die Person hat noch {n} Auftrag/Aufträge und kann nicht gelöscht werden."
            )));
        }
        sqlx::query("DELETE FROM people WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    /// Person über den Namen finden (ohne Groß-/Kleinschreibung) oder neu anlegen.
    async fn person_by_name(&self, name: &str) -> Result<String> {
        let name = name.trim();
        if let Some(row) = sqlx::query("SELECT id FROM people WHERE LOWER(name) = LOWER($1)")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?
        {
            return Ok(row.try_get("id")?);
        }
        self.save_person(PersonInput { id: None, name: name.into(), contact: String::new(), note: String::new() })
            .await
    }

    // ---------- Aufträge ----------

    pub async fn list_orders(&self, search: Option<&str>) -> Result<Vec<OrderSummary>> {
        let q = search.map(str::trim).filter(|s| !s.is_empty());
        let rows = match q {
            None => {
                sqlx::query_as::<_, OrderSummary>(&format!("{ORDER_SUMMARY_SQL} ORDER BY o.status, o.position"))
                    .fetch_all(&self.pool)
                    .await?
            }
            Some(q) => {
                let like = format!("%{}%", q.to_lowercase());
                sqlx::query_as::<_, OrderSummary>(&format!(
                    "{ORDER_SUMMARY_SQL}
                     WHERE LOWER(o.title) LIKE $1 OR LOWER(o.description) LIKE $1
                        OR LOWER(COALESCE(p.name, '')) LIKE $1 OR CAST(o.number AS TEXT) = $2
                     ORDER BY o.status, o.position"
                ))
                .bind(like)
                .bind(q.trim_start_matches('#').to_string())
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows)
    }

    pub async fn get_order(&self, id: &str) -> Result<Order> {
        sqlx::query_as::<_, Order>(
            "SELECT id, number, person_id, title, description, status, priority, due_date, position,
                    archived_at, created_at, updated_at FROM orders WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Auftrag".into()))
    }

    pub async fn order_detail(&self, id: &str, folder: String) -> Result<OrderDetail> {
        let order = self.get_order(id).await?;
        let person = match &order.person_id {
            Some(pid) => self.get_person(pid).await?,
            None => None,
        };
        let items = sqlx::query_as::<_, Item>(
            "SELECT id, order_id, position, title, quantity, color_wish, note, catalog_id, done, file_attachment_id
             FROM order_items WHERE order_id = $1 ORDER BY position",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let attachments = sqlx::query_as::<_, Attachment>(
            "SELECT id, order_id, kind, name, path, url, body, created_at
             FROM attachments WHERE order_id = $1 ORDER BY created_at",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let events = sqlx::query_as::<_, Event>(
            "SELECT id, order_id, kind, from_status, to_status, body, created_at
             FROM events WHERE order_id = $1 ORDER BY created_at DESC",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        let runs = self.list_runs(id).await?;
        Ok(OrderDetail { order, person, items, attachments, events, runs, folder })
    }

    async fn next_position(&self, status: &str) -> Result<i64> {
        let row = sqlx::query("SELECT COALESCE(MAX(position), 0) + 1 AS p FROM orders WHERE status = $1")
            .bind(status)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.try_get("p")?)
    }

    pub async fn create_order(&self, input: OrderInput) -> Result<Order> {
        let title = input.title.trim();
        if title.is_empty() {
            return Err(AppError::Invalid("Bitte einen Titel angeben.".into()));
        }
        let person_id = match (input.person_id, input.person_name.as_deref().map(str::trim)) {
            (Some(id), _) if !id.is_empty() => Some(id),
            (_, Some(name)) if !name.is_empty() => Some(self.person_by_name(name).await?),
            _ => None,
        };
        let mut tx = self.pool.begin().await?;
        let number: i64 = sqlx::query("SELECT COALESCE(MAX(number), 0) + 1 AS n FROM orders")
            .fetch_one(&mut *tx)
            .await?
            .try_get("n")?;
        let position: i64 = sqlx::query("SELECT COALESCE(MAX(position), 0) + 1 AS p FROM orders WHERE status = 'eingang'")
            .fetch_one(&mut *tx)
            .await?
            .try_get("p")?;
        let id = new_id();
        let ts = now();
        sqlx::query(
            "INSERT INTO orders (id, number, person_id, title, description, status, priority, due_date,
                                 position, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, 'eingang', $6, $7, $8, $9, $9)",
        )
        .bind(&id)
        .bind(number)
        .bind(person_id)
        .bind(title)
        .bind(input.description.trim())
        .bind(input.priority)
        .bind(input.due_date.filter(|d| !d.is_empty()))
        .bind(position)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO events (id, order_id, kind, to_status, body, created_at)
             VALUES ($1, $2, 'created', 'eingang', '', $3)",
        )
        .bind(new_id())
        .bind(&id)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.get_order(&id).await
    }

    pub async fn update_order(&self, u: OrderUpdate) -> Result<()> {
        let title = u.title.trim();
        if title.is_empty() {
            return Err(AppError::Invalid("Bitte einen Titel angeben.".into()));
        }
        let r = sqlx::query(
            "UPDATE orders SET title = $1, description = $2, person_id = $3, priority = $4, due_date = $5,
                               updated_at = $6 WHERE id = $7",
        )
        .bind(title)
        .bind(u.description.trim())
        .bind(u.person_id.filter(|p| !p.is_empty()))
        .bind(u.priority)
        .bind(u.due_date.filter(|d| !d.is_empty()))
        .bind(now())
        .bind(&u.id)
        .execute(&self.pool)
        .await?;
        if r.rows_affected() == 0 {
            return Err(AppError::NotFound("Auftrag".into()));
        }
        Ok(())
    }

    pub async fn set_status(&self, id: &str, status: &str, note: Option<&str>) -> Result<()> {
        if !valid_status(status) {
            return Err(AppError::Invalid(format!("Unbekannter Status: {status}")));
        }
        let order = self.get_order(id).await?;
        if order.status == status {
            return Ok(());
        }
        let position = self.next_position(status).await?;
        let ts = now();
        let mut tx = self.pool.begin().await?;
        // Ein neuer Status holt den Auftrag aus dem Archiv zurück.
        sqlx::query("UPDATE orders SET status = $1, position = $2, updated_at = $3, archived_at = NULL WHERE id = $4")
            .bind(status)
            .bind(position)
            .bind(&ts)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO events (id, order_id, kind, from_status, to_status, body, created_at)
             VALUES ($1, $2, 'status', $3, $4, $5, $6)",
        )
        .bind(new_id())
        .bind(id)
        .bind(&order.status)
        .bind(status)
        .bind(note.unwrap_or("").trim())
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Innerhalb der Spalte nach oben (-1) oder unten (+1) verschieben.
    pub async fn move_order(&self, id: &str, direction: i64) -> Result<()> {
        let o = self.get_order(id).await?;
        let sql = if direction < 0 {
            "SELECT id, position FROM orders WHERE status = $1 AND position < $2 ORDER BY position DESC LIMIT 1"
        } else {
            "SELECT id, position FROM orders WHERE status = $1 AND position > $2 ORDER BY position ASC LIMIT 1"
        };
        let Some(other) = sqlx::query(sql).bind(&o.status).bind(o.position).fetch_optional(&self.pool).await? else {
            return Ok(());
        };
        let other_id: String = other.try_get("id")?;
        let other_pos: i64 = other.try_get("position")?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE orders SET position = $1 WHERE id = $2")
            .bind(other_pos)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE orders SET position = $1 WHERE id = $2")
            .bind(o.position)
            .bind(&other_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_order(&self, id: &str) -> Result<()> {
        // Kindtabellen ausdrücklich löschen, falls Fremdschlüssel nicht aktiv sind
        let mut tx = self.pool.begin().await?;
        for t in ["events", "attachments", "order_items"] {
            sqlx::query(&format!("DELETE FROM {t} WHERE order_id = $1")).bind(id).execute(&mut *tx).await?;
        }
        sqlx::query("DELETE FROM orders WHERE id = $1").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn touch(&self, order_id: &str) -> Result<()> {
        sqlx::query("UPDATE orders SET updated_at = $1 WHERE id = $2")
            .bind(now())
            .bind(order_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---------- Teile ----------

    pub async fn save_item(&self, it: Item) -> Result<String> {
        if it.title.trim().is_empty() {
            return Err(AppError::Invalid("Bitte eine Bezeichnung für das Teil angeben.".into()));
        }
        let qty = it.quantity.max(1);
        let id = if it.id.is_empty() {
            let id = new_id();
            let pos: i64 = sqlx::query("SELECT COALESCE(MAX(position), 0) + 1 AS p FROM order_items WHERE order_id = $1")
                .bind(&it.order_id)
                .fetch_one(&self.pool)
                .await?
                .try_get("p")?;
            sqlx::query(
                "INSERT INTO order_items (id, order_id, position, title, quantity, color_wish, note, catalog_id, done,
                                          file_attachment_id)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(&id)
            .bind(&it.order_id)
            .bind(pos)
            .bind(it.title.trim())
            .bind(qty)
            .bind(it.color_wish.trim())
            .bind(it.note.trim())
            .bind(it.catalog_id)
            .bind(it.done.clamp(0, 1))
            .bind(it.file_attachment_id)
            .execute(&self.pool)
            .await?;
            id
        } else {
            sqlx::query(
                "UPDATE order_items SET title = $1, quantity = $2, color_wish = $3, note = $4, done = $5
                 WHERE id = $6",
            )
            .bind(it.title.trim())
            .bind(qty)
            .bind(it.color_wish.trim())
            .bind(it.note.trim())
            .bind(it.done.clamp(0, 1))
            .bind(&it.id)
            .execute(&self.pool)
            .await?;
            it.id
        };
        self.touch(&it.order_id).await?;
        Ok(id)
    }

    pub async fn delete_item(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM order_items WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ---------- Anhänge und Verlauf ----------

    #[allow(clippy::too_many_arguments)]
    pub async fn add_attachment(
        &self,
        order_id: &str,
        kind: &str,
        name: &str,
        path: Option<&str>,
        url: Option<&str>,
        body: Option<&str>,
    ) -> Result<String> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO attachments (id, order_id, kind, name, path, url, body, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&id)
        .bind(order_id)
        .bind(kind)
        .bind(name)
        .bind(path)
        .bind(url)
        .bind(body)
        .bind(now())
        .execute(&self.pool)
        .await?;
        self.touch(order_id).await?;
        Ok(id)
    }

    pub async fn get_attachment(&self, id: &str) -> Result<Attachment> {
        sqlx::query_as::<_, Attachment>(
            "SELECT id, order_id, kind, name, path, url, body, created_at FROM attachments WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Anhang".into()))
    }

    pub async fn delete_attachment(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE order_items SET file_attachment_id = NULL WHERE file_attachment_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM attachments WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ---------- Katalog ----------

    pub async fn list_catalog(&self, include_hidden: bool) -> Result<Vec<CatalogEntry>> {
        let mut list = sqlx::query_as::<_, CatalogEntry>(&format!(
            "SELECT c.id, c.title, c.description, c.file_name, c.thumb_name, c.source_url, c.designer, c.license,
                    c.plates, c.print_seconds, c.weight_grams, c.filaments, c.tags, c.hidden, c.created_at, c.updated_at,
                    (SELECT COUNT(*) FROM order_items i WHERE i.catalog_id = c.id) AS usage_count,
                    c.origin, c.remote_id, c.cover_url, c.remote_missing
             FROM catalog c {} ORDER BY LOWER(c.title)",
            if include_hidden { "" } else { "WHERE c.hidden = 0" }
        ))
        .fetch_all(&self.pool)
        .await?;
        let links = sqlx::query("SELECT catalog_id, collection_id FROM catalog_collections").fetch_all(&self.pool).await?;
        let mut map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for r in links {
            map.entry(r.try_get("catalog_id")?).or_default().push(r.try_get("collection_id")?);
        }
        for e in &mut list {
            e.collection_ids = map.remove(&e.id).unwrap_or_default();
        }
        Ok(list)
    }

    pub async fn get_catalog(&self, id: &str) -> Result<CatalogEntry> {
        sqlx::query_as::<_, CatalogEntry>(
            "SELECT c.id, c.title, c.description, c.file_name, c.thumb_name, c.source_url, c.designer, c.license,
                    c.plates, c.print_seconds, c.weight_grams, c.filaments, c.tags, c.hidden, c.created_at, c.updated_at,
                    (SELECT COUNT(*) FROM order_items i WHERE i.catalog_id = c.id) AS usage_count,
                    c.origin, c.remote_id, c.cover_url, c.remote_missing
             FROM catalog c WHERE c.id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Katalogprojekt".into()))
    }

    /// Neue ID vorab, damit der Ordner `catalog/<id>/` schon vor dem Eintrag befüllt werden kann.
    pub fn new_catalog_id() -> String {
        new_id()
    }

    pub async fn insert_catalog(&self, id: &str, e: NewCatalogEntry) -> Result<()> {
        let ts = now();
        sqlx::query(
            "INSERT INTO catalog (id, title, description, file_name, thumb_name, source_url, designer, license,
                                  plates, print_seconds, weight_grams, filaments, tags, hidden, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, '', 0, $13, $13)",
        )
        .bind(id)
        .bind(if e.title.trim().is_empty() { e.file_name.clone() } else { e.title.trim().to_string() })
        .bind(e.description)
        .bind(e.file_name)
        .bind(e.thumb_name)
        .bind(e.source_url)
        .bind(e.designer)
        .bind(e.license)
        .bind(e.plates.max(1))
        .bind(e.print_seconds)
        .bind(e.weight_grams)
        .bind(e.filaments)
        .bind(ts)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_catalog(&self, u: CatalogUpdate) -> Result<()> {
        if u.title.trim().is_empty() {
            return Err(AppError::Invalid("Der Titel darf nicht leer sein.".into()));
        }
        sqlx::query("UPDATE catalog SET title = $1, description = $2, tags = $3, hidden = $4, updated_at = $5 WHERE id = $6")
            .bind(u.title.trim())
            .bind(u.description.trim())
            .bind(u.tags.trim())
            .bind(u.hidden.clamp(0, 1))
            .bind(now())
            .bind(&u.id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_catalog(&self, id: &str) -> Result<()> {
        let e = self.get_catalog(id).await?;
        if e.usage_count > 0 {
            return Err(AppError::Invalid(
                "Das Projekt steckt schon in Aufträgen. Blende es stattdessen aus.".into(),
            ));
        }
        sqlx::query("DELETE FROM catalog WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn add_note(&self, order_id: &str, body: &str) -> Result<()> {
        let body = body.trim();
        if body.is_empty() {
            return Ok(());
        }
        sqlx::query("INSERT INTO events (id, order_id, kind, body, created_at) VALUES ($1, $2, 'note', $3, $4)")
            .bind(new_id())
            .bind(order_id)
            .bind(body)
            .bind(now())
            .execute(&self.pool)
            .await?;
        self.touch(order_id).await
    }
}

// ---------- Filament und Drucke ----------

const SPOOL_COLS: &str = "id, brand, series, material, color, color_name, initial_grams, studio_grams,
    remaining_grams, studio_status, hidden, missing, note, updated_at, source, source_updated_at, in_printer,
    printer_name, ams_slot, depleted";
const RUN_COLS: &str = "id, order_id, item_id, attachment_id, file_name, plates, print_seconds, weight_grams,
    filaments, result, deducted, note, created_at, updated_at, printer_name, printer_status, printer_finished_at,
    actual_seconds, actual_filaments";

fn fmt_grams(g: f64) -> String {
    if g >= 10.0 { format!("{g:.0} g") } else { format!("{g:.1} g") }
}

impl Store {
    pub async fn order_by_number(&self, number: i64) -> Result<Option<Order>> {
        Ok(sqlx::query_as::<_, Order>(
            "SELECT id, number, person_id, title, description, status, priority, due_date, position,
                    archived_at, created_at, updated_at FROM orders WHERE number = $1",
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn attachment_by_path(&self, order_id: &str, path: &str) -> Result<Option<Attachment>> {
        Ok(sqlx::query_as::<_, Attachment>(
            "SELECT id, order_id, kind, name, path, url, body, created_at FROM attachments
             WHERE order_id = $1 AND path = $2 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(order_id)
        .bind(path)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_spools(&self) -> Result<Vec<Spool>> {
        Ok(sqlx::query_as::<_, Spool>(&format!(
            "SELECT {SPOOL_COLS} FROM spools ORDER BY material, brand, series, color"
        ))
        .fetch_all(&self.pool)
        .await?)
    }

    /// Spulen aus Bambu Studio übernehmen. Neue Spulen starten mit dem Rest aus Studio.
    /// Hat sich der Rest in Studio geändert (dort bearbeitet), gilt der neue Wert aus Studio;
    /// sonst bleibt der Rest der App (inkl. der hier abgezogenen Gramm) stehen.
    pub async fn sync_spools(&self, list: &[StudioSpool]) -> Result<SyncResult> {
        let mut r = SyncResult { found: true, source: list.first().map(|s| s.source.clone()).unwrap_or_default(), ..Default::default() };
        let ts = now();
        let mut tx = self.pool.begin().await?;
        for sp in list {
            let row = sqlx::query("SELECT studio_grams, source, source_updated_at FROM spools WHERE id = $1")
                .bind(&sp.id)
                .fetch_optional(&mut *tx)
                .await?;
            match row {
                None => {
                    sqlx::query(
                        "INSERT INTO spools (id, brand, series, material, color, color_name, initial_grams,
                            studio_grams, remaining_grams, studio_status, hidden, missing, note, created_at, updated_at,
                            source, source_updated_at, in_printer, printer_name, ams_slot, depleted)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, 0, '', $11, $12, $13, $14, $15, $16, $17, $18)",
                    )
                    .bind(&sp.id)
                    .bind(&sp.brand)
                    .bind(&sp.series)
                    .bind(&sp.material)
                    .bind(&sp.color)
                    .bind(&sp.color_name)
                    .bind(sp.initial_grams)
                    .bind(sp.net_grams)
                    .bind(sp.net_grams)
                    .bind(&sp.status)
                    .bind(&ts)
                    .bind(&ts)
                    .bind(&sp.source)
                    .bind(&sp.source_updated_at)
                    .bind(sp.in_printer as i64)
                    .bind(&sp.printer_name)
                    .bind(&sp.ams_slot)
                    .bind(sp.depleted as i64)
                    .execute(&mut *tx)
                    .await?;
                    r.added += 1;
                }
                Some(row) => {
                    // Eine ältere spools.json überschreibt keine Werte, die schon aus dem Konto kamen
                    // (z.B. nach dem Abmelden). Sie gilt erst wieder, wenn die Datei neuer ist.
                    let prev_source: String = row.try_get("source")?;
                    let prev_at: Option<String> = row.try_get("source_updated_at")?;
                    if sp.source == "datei" && prev_source == "konto" && sp.source_updated_at <= prev_at {
                        continue;
                    }
                    let studio: f64 = row.try_get("studio_grams")?;
                    let changed = (studio - sp.net_grams).abs() > 0.05;
                    let sql = if changed {
                        "UPDATE spools SET brand = $1, series = $2, material = $3, color = $4, color_name = $5,
                            initial_grams = $6, studio_grams = $7, studio_status = $8, missing = 0,
                            remaining_grams = $7, updated_at = $9, source = $11, source_updated_at = $12,
                            in_printer = $13, printer_name = $14, ams_slot = $15, depleted = $16 WHERE id = $10"
                    } else {
                        "UPDATE spools SET brand = $1, series = $2, material = $3, color = $4, color_name = $5,
                            initial_grams = $6, studio_grams = $7, studio_status = $8, missing = 0,
                            updated_at = $9, source = $11, source_updated_at = $12,
                            in_printer = $13, printer_name = $14, ams_slot = $15, depleted = $16 WHERE id = $10"
                    };
                    sqlx::query(sql)
                        .bind(&sp.brand)
                        .bind(&sp.series)
                        .bind(&sp.material)
                        .bind(&sp.color)
                        .bind(&sp.color_name)
                        .bind(sp.initial_grams)
                        .bind(sp.net_grams)
                        .bind(&sp.status)
                        .bind(&ts)
                        .bind(&sp.id)
                        .bind(&sp.source)
                        .bind(&sp.source_updated_at)
                        .bind(sp.in_printer as i64)
                        .bind(&sp.printer_name)
                        .bind(&sp.ams_slot)
                        .bind(sp.depleted as i64)
                        .execute(&mut *tx)
                        .await?;
                    if changed {
                        r.updated += 1;
                    }
                }
            }
        }
        // Spulen, die in Studio gelöscht wurden, markieren (nicht löschen: Drucke verweisen darauf)
        let ids: Vec<String> = sqlx::query("SELECT id FROM spools WHERE missing = 0")
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(|r| r.get::<String, _>("id"))
            .collect();
        for id in ids.iter().filter(|id| !list.iter().any(|s| &s.id == *id)) {
            sqlx::query("UPDATE spools SET missing = 1, in_printer = 0 WHERE id = $1").bind(id).execute(&mut *tx).await?;
            r.missing += 1;
        }
        tx.commit().await?;
        Ok(r)
    }

    pub async fn update_spool(&self, u: SpoolUpdate) -> Result<()> {
        if !u.remaining_grams.is_finite() || u.remaining_grams < 0.0 {
            return Err(AppError::Invalid("Der Rest muss 0 g oder mehr sein.".into()));
        }
        let n = sqlx::query(
            "UPDATE spools SET remaining_grams = $1, hidden = $2, note = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(u.remaining_grams)
        .bind(u.hidden)
        .bind(u.note.trim())
        .bind(now())
        .bind(&u.id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if n == 0 {
            return Err(AppError::NotFound("Spule".into()));
        }
        Ok(())
    }

    pub async fn list_runs(&self, order_id: &str) -> Result<Vec<PrintRun>> {
        Ok(sqlx::query_as::<_, PrintRun>(&format!(
            "SELECT {RUN_COLS} FROM print_runs WHERE order_id = $1 ORDER BY created_at DESC"
        ))
        .bind(order_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_run(&self, id: &str) -> Result<PrintRun> {
        sqlx::query_as::<_, PrintRun>(&format!("SELECT {RUN_COLS} FROM print_runs WHERE id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Druck".into()))
    }

    /// Signatur des letzten erkannten Drucks einer Datei (um erneutes Speichern ohne Slicen zu erkennen).
    pub async fn last_run_signature(&self, order_id: &str, file_name: &str) -> Result<Option<String>> {
        Ok(sqlx::query(
            "SELECT signature FROM print_runs WHERE order_id = $1 AND file_name = $2 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(order_id)
        .bind(file_name)
        .fetch_optional(&self.pool)
        .await?
        .map(|r| r.get::<String, _>("signature")))
    }

    pub async fn insert_run(&self, n: NewRun) -> Result<String> {
        let id = new_id();
        let ts = now();
        let item_id = match n.item_id {
            Some(i) => Some(i),
            None => match &n.attachment_id {
                Some(a) => sqlx::query("SELECT id FROM order_items WHERE file_attachment_id = $1 LIMIT 1")
                    .bind(a)
                    .fetch_optional(&self.pool)
                    .await?
                    .map(|r| r.get::<String, _>("id")),
                None => None,
            },
        };
        sqlx::query(
            "INSERT INTO print_runs (id, order_id, item_id, attachment_id, file_name, plates, print_seconds,
                weight_grams, filaments, signature, result, deducted, note, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'offen', 0, '', $11, $12)",
        )
        .bind(&id)
        .bind(&n.order_id)
        .bind(&item_id)
        .bind(&n.attachment_id)
        .bind(&n.file_name)
        .bind(n.plates)
        .bind(n.print_seconds)
        .bind(n.weight_grams)
        .bind(serde_json::to_string(&n.filaments).unwrap_or_else(|_| "[]".into()))
        .bind(&n.signature)
        .bind(&ts)
        .bind(&ts)
        .execute(&self.pool)
        .await?;
        let total: f64 = n.filaments.iter().map(|f| f.grams).sum();
        let mut body = format!("Gesliced gespeichert: {}", n.file_name);
        if total > 0.0 {
            body.push_str(&format!(" ({})", fmt_grams(total)));
        }
        self.add_event(&n.order_id, "print", &body).await?;
        Ok(id)
    }

    async fn add_event(&self, order_id: &str, kind: &str, body: &str) -> Result<()> {
        sqlx::query("INSERT INTO events (id, order_id, kind, body, created_at) VALUES ($1, $2, $3, $4, $5)")
            .bind(new_id())
            .bind(order_id)
            .bind(kind)
            .bind(body)
            .bind(now())
            .execute(&self.pool)
            .await?;
        self.touch(order_id).await
    }

    /// Ergebnis eines Drucks setzen. „ok“ hakt das zugehörige Teil als erledigt ab.
    pub async fn set_run_result(&self, id: &str, result: &str, note: &str) -> Result<()> {
        if !RUN_RESULTS.contains(&result) {
            return Err(AppError::Invalid(format!("Unbekanntes Ergebnis: {result}")));
        }
        let run = self.get_run(id).await?;
        sqlx::query("UPDATE print_runs SET result = $1, note = $2, updated_at = $3 WHERE id = $4")
            .bind(result)
            .bind(note.trim())
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if run.result != result {
            if let (Some(item), "ok") = (&run.item_id, result) {
                sqlx::query("UPDATE order_items SET done = 1 WHERE id = $1").bind(item).execute(&self.pool).await?;
            }
            let label = match result {
                "ok" => "Druck erfolgreich",
                "fehldruck" => "Fehldruck",
                _ => "Druck wieder offen",
            };
            let body = if note.trim().is_empty() {
                format!("{label}: {}", run.file_name)
            } else {
                format!("{label}: {} – {}", run.file_name, note.trim())
            };
            self.add_event(&run.order_id, "print", &body).await?;
        }
        Ok(())
    }

    /// Verbrauch eines Drucks von den Spulen abziehen.
    pub async fn deduct_run(&self, id: &str, list: &[Deduction]) -> Result<()> {
        let run = self.get_run(id).await?;
        if run.deducted != 0 {
            return Err(AppError::Invalid("Der Verbrauch dieses Drucks wurde schon abgezogen.".into()));
        }
        let mut fils: Vec<RunFilament> = serde_json::from_str(&run.filaments).unwrap_or_default();
        let ts = now();
        let mut parts = Vec::new();
        let mut tx = self.pool.begin().await?;
        for d in list {
            let Some(f) = fils.get_mut(d.index) else {
                return Err(AppError::Invalid("Unbekanntes Filament im Druck".into()));
            };
            if !d.grams.is_finite() || d.grams < 0.0 {
                return Err(AppError::Invalid("Gramm müssen 0 oder mehr sein.".into()));
            }
            f.grams = (d.grams * 100.0).round() / 100.0;
            f.spool_id = d.spool_id.clone().filter(|s| !s.is_empty());
            if let Some(sid) = &f.spool_id {
                let n = sqlx::query(
                    "UPDATE spools SET remaining_grams = CASE WHEN remaining_grams > $1 THEN remaining_grams - $1 ELSE 0 END,
                        updated_at = $2 WHERE id = $3",
                )
                .bind(f.grams)
                .bind(&ts)
                .bind(sid)
                .execute(&mut *tx)
                .await?
                .rows_affected();
                if n == 0 {
                    return Err(AppError::NotFound("Spule".into()));
                }
                parts.push(format!("{} {}", fmt_grams(f.grams), f.kind));
            }
        }
        sqlx::query("UPDATE print_runs SET filaments = $1, deducted = 1, updated_at = $2 WHERE id = $3")
            .bind(serde_json::to_string(&fils).unwrap_or_else(|_| "[]".into()))
            .bind(&ts)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        if !parts.is_empty() {
            self.add_event(&run.order_id, "print", &format!("Verbrauch abgezogen: {}", parts.join(", "))).await?;
        }
        Ok(())
    }

    /// Abzug zurücknehmen: Gramm wieder auf die Spulen buchen.
    pub async fn undo_deduct(&self, id: &str) -> Result<()> {
        let run = self.get_run(id).await?;
        if run.deducted == 0 {
            return Ok(());
        }
        let fils: Vec<RunFilament> = serde_json::from_str(&run.filaments).unwrap_or_default();
        let ts = now();
        let mut tx = self.pool.begin().await?;
        for f in &fils {
            if let Some(sid) = &f.spool_id {
                sqlx::query("UPDATE spools SET remaining_grams = remaining_grams + $1, updated_at = $2 WHERE id = $3")
                    .bind(f.grams)
                    .bind(&ts)
                    .bind(sid)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        sqlx::query("UPDATE print_runs SET deducted = 0, updated_at = $1 WHERE id = $2")
            .bind(&ts)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.add_event(&run.order_id, "print", &format!("Abzug zurückgenommen: {}", run.file_name)).await
    }

    pub async fn delete_run(&self, id: &str) -> Result<()> {
        self.undo_deduct(id).await?;
        sqlx::query("DELETE FROM print_runs WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }
}

// ---------- Archiv und Statistik ----------

/// Status, in denen ein Auftrag archiviert werden kann
pub const ARCHIVABLE: [&str; 2] = ["uebergeben", "abgelehnt"];

impl Store {
    /// Auftrag archivieren (`true`) oder zurückholen (`false`).
    pub async fn set_archived(&self, id: &str, archived: bool) -> Result<()> {
        let o = self.get_order(id).await?;
        if archived && !ARCHIVABLE.contains(&o.status.as_str()) {
            return Err(AppError::Invalid("Archivieren geht nur bei übergebenen oder abgelehnten Aufträgen.".into()));
        }
        if o.archived_at.is_some() == archived {
            return Ok(());
        }
        let ts = now();
        sqlx::query("UPDATE orders SET archived_at = $1, updated_at = $2 WHERE id = $3")
            .bind(archived.then(|| ts.clone()))
            .bind(&ts)
            .bind(id)
            .execute(&self.pool)
            .await?;
        self.add_event(id, "edit", if archived { "Archiviert" } else { "Aus dem Archiv geholt" }).await
    }

    /// Alle übergebenen und abgelehnten Aufträge archivieren, die seit `days` Tagen unverändert sind.
    pub async fn archive_closed(&self, days: i64) -> Result<u64> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let ids: Vec<String> = sqlx::query(
            "SELECT id FROM orders WHERE archived_at IS NULL AND status IN ('uebergeben', 'abgelehnt') AND updated_at <= $1",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| r.try_get("id"))
        .collect::<std::result::Result<_, _>>()?;
        for id in &ids {
            self.set_archived(id, true).await?;
        }
        Ok(ids.len() as u64)
    }

    /// Rohdaten für die Statistik: alle Drucke und alle Übergaben. Zusammengefasst wird in der Oberfläche.
    pub async fn stats(&self) -> Result<Stats> {
        let runs = sqlx::query_as::<_, StatRun>(
            "SELECT r.created_at, r.result, COALESCE(r.actual_seconds, r.print_seconds) AS print_seconds, r.filaments, o.number AS order_number,
                    p.name AS person_name
             FROM print_runs r JOIN orders o ON o.id = r.order_id LEFT JOIN people p ON p.id = o.person_id
             ORDER BY r.created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        let handovers = sqlx::query_as::<_, StatHandover>(
            "SELECT e.created_at, o.number AS order_number, p.name AS person_name
             FROM events e JOIN orders o ON o.id = e.order_id LEFT JOIN people p ON p.id = o.person_id
             WHERE e.kind = 'status' AND e.to_status = 'uebergeben'
             ORDER BY e.created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(Stats { runs, handovers })
    }
}

// ---------- Drucker (Bambu-Konto) ----------

/// Ein fertiger Eintrag aus dem Druckverlauf des Kontos
#[derive(Clone, Copy)]
pub struct PrinterTask<'a> {
    pub id: &'a str,
    pub printer: &'a str,
    /// ok | fehldruck
    pub status: &'a str,
    pub ended_at: &'a str,
    pub actual_seconds: Option<i64>,
    pub actual: &'a [RunFilament],
}

impl Store {
    /// Offene Aufträge (nicht archiviert, nicht übergeben/abgelehnt) mit Namen ihrer 3MF-Dateien und Drucke.
    pub async fn open_order_files(&self) -> Result<Vec<OrderFiles>> {
        let rows = sqlx::query(
            "SELECT o.id, o.number, o.title, a.name AS file FROM orders o
               JOIN attachments a ON a.order_id = o.id AND a.kind = 'file'
              WHERE o.archived_at IS NULL AND o.status NOT IN ('uebergeben', 'abgelehnt')
             UNION
             SELECT o.id, o.number, o.title, r.file_name AS file FROM orders o
               JOIN print_runs r ON r.order_id = o.id
              WHERE o.archived_at IS NULL AND o.status NOT IN ('uebergeben', 'abgelehnt')",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut out: Vec<OrderFiles> = Vec::new();
        for r in rows {
            let id: String = r.try_get("id")?;
            let file: String = r.try_get("file")?;
            match out.iter_mut().find(|o| o.order_id == id) {
                Some(o) => o.files.push(file),
                None => out.push(OrderFiles { order_id: id, number: r.try_get("number")?, title: r.try_get("title")?, files: vec![file] }),
            }
        }
        Ok(out)
    }

    /// Rückmeldung des Druckers an den passenden Druck hängen: der neueste Druck des Auftrags,
    /// dessen Datei passt, sonst der neueste ohne Rückmeldung. Gibt die Druck-ID zurück.
    pub async fn record_printer_result(
        &self,
        order_id: &str,
        file_hint: Option<&str>,
        task: &PrinterTask<'_>,
    ) -> Result<Option<String>> {
        let PrinterTask { id: task_id, printer, status, ended_at, actual_seconds, actual } = *task;
        let used: Option<String> = sqlx::query("SELECT id FROM print_runs WHERE printer_task_id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.get("id"));
        if used.is_some() {
            return Ok(None);
        }
        let ended = chrono::DateTime::parse_from_rfc3339(ended_at).ok();
        let runs = self.list_runs(order_id).await?;
        let open: Vec<&PrintRun> = runs
            .iter()
            .filter(|r| r.printer_status.is_empty())
            .filter(|r| chrono::DateTime::parse_from_rfc3339(&r.created_at).ok() < ended)
            .collect();
        let pick = file_hint.and_then(|f| open.iter().find(|r| r.file_name == f)).or(open.first());
        let Some(run) = pick else { return Ok(None) };
        let ts = now();
        sqlx::query(
            "UPDATE print_runs SET printer_name = $1, printer_status = $2, printer_finished_at = $3, actual_seconds = $4,
                    actual_filaments = $5, updated_at = $7, printer_task_id = $8 WHERE id = $6",
        )
        .bind(printer)
        .bind(status)
        .bind(ended_at)
        .bind(actual_seconds)
        .bind(serde_json::to_string(actual).unwrap_or_else(|_| "[]".into()))
        .bind(&run.id)
        .bind(&ts)
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        let label = if status == "ok" { "fertig" } else { "abgebrochen" };
        self.add_event(order_id, "print", &format!("Drucker {printer} meldet: {label} ({})", run.file_name)).await?;
        Ok(Some(run.id.clone()))
    }
}

// ---------- MakerWorld ----------

impl Store {
    pub async fn list_mw_collections(&self) -> Result<Vec<MwCollection>> {
        Ok(sqlx::query_as::<_, MwCollection>(
            "SELECT id, title, design_cnt, is_default, enabled, synced_at FROM mw_collections
             ORDER BY is_default DESC, LOWER(title)",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Sammlungsliste aus MakerWorld übernehmen. Die Auswahl (enabled) bleibt erhalten;
    /// Sammlungen, die es nicht mehr gibt, fallen samt Verknüpfungen weg.
    pub async fn save_mw_collections(&self, list: &[MwCollection]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for c in list {
            let n = sqlx::query("UPDATE mw_collections SET title = $1, design_cnt = $2, is_default = $3 WHERE id = $4")
                .bind(&c.title)
                .bind(c.design_cnt)
                .bind(c.is_default)
                .bind(&c.id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
            if n == 0 {
                sqlx::query("INSERT INTO mw_collections (id, title, design_cnt, is_default, enabled) VALUES ($1, $2, $3, $4, 1)")
                    .bind(&c.id)
                    .bind(&c.title)
                    .bind(c.design_cnt)
                    .bind(c.is_default)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        let ids: Vec<String> = sqlx::query("SELECT id FROM mw_collections").fetch_all(&mut *tx).await?.iter().map(|r| r.get("id")).collect();
        for id in ids.iter().filter(|id| !list.iter().any(|c| &c.id == *id)) {
            sqlx::query("DELETE FROM catalog_collections WHERE collection_id = $1").bind(id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM mw_collections WHERE id = $1").bind(id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_mw_collection_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE mw_collections SET enabled = $1 WHERE id = $2").bind(enabled as i64).bind(id).execute(&self.pool).await?;
        if !enabled {
            sqlx::query("DELETE FROM catalog_collections WHERE collection_id = $1").bind(id).execute(&self.pool).await?;
        }
        Ok(())
    }

    /// Modelle einer Sammlung in den Katalog übernehmen. Bestehende Einträge (auch per Link importierte
    /// mit derselben MakerWorld-Adresse) werden verknüpft statt verdoppelt. Gibt die Zahl neuer Einträge zurück.
    pub async fn save_mw_designs(&self, collection_id: &str, designs: &[MwDesign]) -> Result<i64> {
        let ts = now();
        let mut added = 0;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM catalog_collections WHERE collection_id = $1").bind(collection_id).execute(&mut *tx).await?;
        for d in designs {
            let url = if d.slug.is_empty() {
                format!("https://makerworld.com/en/models/{}", d.id)
            } else {
                format!("https://makerworld.com/en/models/{}-{}", d.id, d.slug)
            };
            let existing: Option<String> = sqlx::query(
                "SELECT id FROM catalog WHERE remote_id = $1
                 OR (remote_id IS NULL AND (source_url LIKE $2 OR source_url LIKE $3)) ORDER BY created_at LIMIT 1",
            )
            .bind(&d.id)
            .bind(format!("%makerworld.com%/models/{}", d.id))
            .bind(format!("%makerworld.com%/models/{}-%", d.id))
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| r.get("id"));
            let id = match existing {
                Some(id) => {
                    sqlx::query(
                        "UPDATE catalog SET remote_id = $1, cover_url = $2, remote_missing = 0, remote_synced_at = $3,
                                designer = CASE WHEN designer = '' THEN $4 ELSE designer END WHERE id = $5",
                    )
                    .bind(&d.id)
                    .bind(&d.cover)
                    .bind(&ts)
                    .bind(&d.designer)
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                    id
                }
                None => {
                    let id = new_id();
                    sqlx::query(
                        "INSERT INTO catalog (id, title, description, file_name, thumb_name, source_url, designer, license,
                            plates, print_seconds, weight_grams, filaments, tags, hidden, created_at, updated_at,
                            origin, remote_id, cover_url, remote_missing, remote_synced_at)
                         VALUES ($1, $2, '', '', NULL, $3, $4, '', 1, NULL, NULL, '[]', '', 0, $5, $5,
                                 'makerworld', $6, $7, 0, $5)",
                    )
                    .bind(&id)
                    .bind(if d.title.trim().is_empty() { format!("MakerWorld {}", d.id) } else { d.title.trim().to_string() })
                    .bind(&url)
                    .bind(&d.designer)
                    .bind(&ts)
                    .bind(&d.id)
                    .bind(&d.cover)
                    .execute(&mut *tx)
                    .await?;
                    added += 1;
                    id
                }
            };
            sqlx::query("INSERT INTO catalog_collections (catalog_id, collection_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(&id)
                .bind(collection_id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE mw_collections SET synced_at = $1 WHERE id = $2").bind(&ts).bind(collection_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(added)
    }

    /// Nach einem vollständigen Abgleich: MakerWorld-Einträge ohne Sammlung markieren (nicht löschen).
    pub async fn mark_mw_missing(&self) -> Result<i64> {
        let r = sqlx::query(
            "UPDATE catalog SET remote_missing = 1 WHERE remote_id IS NOT NULL AND remote_missing = 0
               AND NOT EXISTS (SELECT 1 FROM catalog_collections cc WHERE cc.catalog_id = catalog.id)",
        )
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() as i64)
    }

    /// Nach dem Laden der 3MF: Datei und ausgewertete Angaben eintragen (Titel bleibt, wenn schon gesetzt).
    pub async fn set_catalog_file(&self, id: &str, instance_id: &str, e: &NewCatalogEntry) -> Result<()> {
        sqlx::query(
            "UPDATE catalog SET file_name = $1, thumb_name = COALESCE($2, thumb_name),
                    description = CASE WHEN description = '' THEN $3 ELSE description END,
                    designer = CASE WHEN designer = '' THEN $4 ELSE designer END,
                    license = CASE WHEN $5 <> '' THEN $5 ELSE license END,
                    plates = $6, print_seconds = $7, weight_grams = $8, filaments = $9,
                    remote_instance_id = $10, updated_at = $11 WHERE id = $12",
        )
        .bind(&e.file_name)
        .bind(&e.thumb_name)
        .bind(&e.description)
        .bind(&e.designer)
        .bind(&e.license)
        .bind(e.plates.max(1))
        .bind(e.print_seconds)
        .bind(e.weight_grams)
        .bind(&e.filaments)
        .bind(instance_id)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store() -> (Store, tempfile::TempDir) {
        // Pfad mit Leerzeichen, wie unter macOS "Application Support"
        let dir = tempfile::Builder::new().prefix("pm test ").tempdir().unwrap();
        let s = Store::connect(&Store::sqlite_url(&dir.path().join("t.db"))).await.unwrap();
        (s, dir)
    }

    fn input(title: &str, person: Option<&str>) -> OrderInput {
        OrderInput {
            title: title.into(),
            description: String::new(),
            person_id: None,
            person_name: person.map(Into::into),
            priority: 0,
            due_date: None,
        }
    }

    #[tokio::test]
    async fn auftrag_anlegen_nummer_und_person() {
        let (s, _d) = store().await;
        let a = s.create_order(input("Schlüsselanhänger", Some("Oma"))).await.unwrap();
        let b = s.create_order(input("Vase", Some("oma"))).await.unwrap();
        assert_eq!((a.number, b.number), (1, 2));
        assert_eq!(a.person_id, b.person_id, "gleiche Person trotz anderer Schreibweise");
        assert_eq!(s.list_people().await.unwrap()[0].order_count, 2);
        assert!(s.create_order(input("  ", None)).await.is_err());
    }

    #[tokio::test]
    async fn status_verschieben_und_verlauf() {
        let (s, d) = store().await;
        let a = s.create_order(input("A", None)).await.unwrap();
        let b = s.create_order(input("B", None)).await.unwrap();
        s.move_order(&b.id, -1).await.unwrap();
        let list = s.list_orders(None).await.unwrap();
        assert_eq!(list.iter().map(|o| o.title.as_str()).collect::<Vec<_>>(), ["B", "A"]);

        s.set_status(&a.id, "klaeren", Some("Welche Farbe?")).await.unwrap();
        assert!(s.set_status(&a.id, "quatsch", None).await.is_err());
        let det = s.order_detail(&a.id, d.path().display().to_string()).await.unwrap();
        assert_eq!(det.order.status, "klaeren");
        assert_eq!(det.events[0].kind, "status");
        assert_eq!(det.events[0].body, "Welche Farbe?");
        assert_eq!(det.events.last().unwrap().kind, "created");
    }

    #[tokio::test]
    async fn teile_anhaenge_suche_loeschen() {
        let (s, d) = store().await;
        let a = s.create_order(input("Halter", Some("Max"))).await.unwrap();
        s.save_item(Item {
            id: String::new(),
            order_id: a.id.clone(),
            position: 0,
            title: "Wandhalter".into(),
            quantity: 0,
            color_wish: "blau".into(),
            note: String::new(),
            catalog_id: None,
            done: 0,
            file_attachment_id: None,
        })
        .await
        .unwrap();
        s.add_attachment(&a.id, "image", "foto.jpg", Some("foto.jpg"), None, None).await.unwrap();
        s.add_attachment(&a.id, "link", "MakerWorld", None, Some("https://makerworld.com/x"), None).await.unwrap();
        let stl = s.add_attachment(&a.id, "file", "teil.STL", Some("teil.STL"), None, None).await.unwrap();
        s.add_note(&a.id, "Bis Weihnachten").await.unwrap();

        let list = s.list_orders(Some("max")).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].item_count, list[0].attachment_count), (1, 3));
        assert_eq!(list[0].print_file_id.as_deref(), Some(stl.as_str()), "Druckdatei erkannt");
        assert_eq!(list[0].cover_path.as_deref(), Some("foto.jpg"));
        assert_eq!(s.list_orders(Some("#1")).await.unwrap().len(), 1);
        assert!(s.list_orders(Some("gibtsnicht")).await.unwrap().is_empty());

        let det = s.order_detail(&a.id, d.path().display().to_string()).await.unwrap();
        assert_eq!(det.items[0].quantity, 1, "Anzahl mindestens 1");
        assert!(s.delete_person(det.person.as_ref().unwrap().id.as_str()).await.is_err());

        s.delete_order(&a.id).await.unwrap();
        assert!(s.list_orders(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn katalog_verwenden_und_ausblenden() {
        let (s, _d) = store().await;
        let cid = Store::new_catalog_id();
        s.insert_catalog(&cid, NewCatalogEntry { title: "Vase".into(), file_name: "vase.3mf".into(), filaments: "[]".into(), ..Default::default() })
            .await
            .unwrap();
        let a = s.create_order(input("Geschenk", None)).await.unwrap();
        s.save_item(Item {
            id: String::new(), order_id: a.id.clone(), position: 0, title: "Vase".into(), quantity: 2,
            color_wish: String::new(), note: String::new(), catalog_id: Some(cid.clone()), done: 0, file_attachment_id: None,
        })
        .await
        .unwrap();
        assert_eq!(s.get_catalog(&cid).await.unwrap().usage_count, 1);
        assert!(s.delete_catalog(&cid).await.is_err(), "verwendet: nur ausblenden");
        s.update_catalog(CatalogUpdate { id: cid.clone(), title: "Vase".into(), description: String::new(), tags: String::new(), hidden: 1 })
            .await
            .unwrap();
        assert!(s.list_catalog(false).await.unwrap().is_empty());
        assert_eq!(s.list_catalog(true).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn spulen_und_drucke() {
        let (s, _d) = store().await;
        let sp = |id: &str, net: f64| StudioSpool {
            id: id.into(),
            material: "PLA".into(),
            color: "#BB3D43".into(),
            initial_grams: 1000.0,
            net_grams: net,
            ..Default::default()
        };
        let r = s.sync_spools(&[sp("a", 500.0), sp("b", 1000.0)]).await.unwrap();
        assert_eq!((r.added, r.updated, r.missing), (2, 0, 0));

        let o = s.create_order(input("Figur", None)).await.unwrap();
        let run = s
            .insert_run(NewRun {
                order_id: o.id.clone(),
                file_name: "figur.3mf".into(),
                plates: 1,
                print_seconds: Some(3600),
                weight_grams: Some(12.5),
                filaments: vec![RunFilament { kind: "PLA".into(), color: "#BB3D43".into(), grams: 12.5, spool_id: None }],
                signature: "x".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(s.last_run_signature(&o.id, "figur.3mf").await.unwrap().as_deref(), Some("x"));
        s.deduct_run(&run, &[Deduction { index: 0, spool_id: Some("a".into()), grams: 20.0 }]).await.unwrap();
        assert!(s.deduct_run(&run, &[]).await.is_err(), "nicht doppelt abziehen");
        let rest = |l: Vec<Spool>, id: &str| l.into_iter().find(|x| x.id == id).unwrap().remaining_grams;
        assert_eq!(rest(s.list_spools().await.unwrap(), "a"), 480.0);

        // Erneutes Einlesen ohne Änderung in Studio behält den Rest der App,
        // eine Änderung in Studio gewinnt; fehlende Spulen werden markiert.
        s.sync_spools(&[sp("a", 500.0), sp("b", 1000.0)]).await.unwrap();
        assert_eq!(rest(s.list_spools().await.unwrap(), "a"), 480.0);
        let r = s.sync_spools(&[sp("a", 300.0)]).await.unwrap();
        assert_eq!((r.updated, r.missing), (1, 1));
        assert_eq!(rest(s.list_spools().await.unwrap(), "a"), 300.0);

        s.undo_deduct(&run).await.unwrap();
        assert_eq!(rest(s.list_spools().await.unwrap(), "a"), 320.0);
        s.set_run_result(&run, "ok", "").await.unwrap();
        assert!(s.set_run_result(&run, "kaputt", "").await.is_err());
        let d = s.order_detail(&o.id, String::new()).await.unwrap();
        assert_eq!(d.runs[0].result, "ok");
        assert!(d.events.iter().any(|e| e.kind == "print"));
        s.delete_run(&run).await.unwrap();
        assert!(s.list_runs(&o.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn archiv_und_statistik() {
        let (s, _d) = store().await;
        let o = s.create_order(input("Vase", Some("Oma"))).await.unwrap();
        assert!(s.set_archived(&o.id, true).await.is_err(), "offene Aufträge nicht archivieren");
        s.set_status(&o.id, "uebergeben", None).await.unwrap();
        assert_eq!(s.archive_closed(1).await.unwrap(), 0, "erst nach einem Tag");
        assert_eq!(s.archive_closed(0).await.unwrap(), 1);
        let l = s.list_orders(None).await.unwrap();
        assert!(l[0].archived_at.is_some());
        // Neuer Status holt den Auftrag zurück
        s.set_status(&o.id, "fertig", None).await.unwrap();
        assert!(s.get_order(&o.id).await.unwrap().archived_at.is_none());
        let st = s.stats().await.unwrap();
        assert_eq!(st.handovers.len(), 1);
        assert_eq!(st.handovers[0].person_name.as_deref(), Some("Oma"));
    }

    #[tokio::test]
    async fn spulen_aus_konto_und_datei() {
        let (s, _d) = store().await;
        let sp = |src: &str, net: f64, at: &str| StudioSpool {
            id: "1".into(),
            material: "PLA".into(),
            initial_grams: 1000.0,
            net_grams: net,
            source: src.into(),
            source_updated_at: Some(at.into()),
            ..Default::default()
        };
        // Datei vom 08.11.: 500 g
        s.sync_spools(&[sp("datei", 500.0, "2026-11-08T18:01:00Z")]).await.unwrap();
        // Konto: 650 g, steckt im AMS
        let mut k = sp("konto", 650.0, "2026-11-16T08:27:03Z");
        k.in_printer = true;
        k.printer_name = "h2c".into();
        k.ams_slot = "AMS 1 · Slot 2".into();
        let r = s.sync_spools(&[k.clone()]).await.unwrap();
        assert_eq!((r.updated, r.source.as_str()), (1, "konto"));
        let l = s.list_spools().await.unwrap();
        assert_eq!((l[0].remaining_grams, l[0].in_printer, l[0].ams_slot.as_str()), (650.0, 1, "AMS 1 · Slot 2"));

        // Ältere Datei (z.B. nach dem Abmelden) überschreibt den Kontowert nicht
        s.sync_spools(&[sp("datei", 500.0, "2026-11-08T18:01:00Z")]).await.unwrap();
        assert_eq!(s.list_spools().await.unwrap()[0].remaining_grams, 650.0);
        // Neuere Datei schon
        s.sync_spools(&[sp("datei", 400.0, "2026-11-20T10:00:00Z")]).await.unwrap();
        let l = s.list_spools().await.unwrap();
        assert_eq!((l[0].remaining_grams, l[0].source.as_str(), l[0].in_printer), (400.0, "datei", 0));

        // Aus dem Konto verschwunden: markiert, nicht mehr im Drucker
        let mut other = k.clone();
        other.id = "2".into();
        let r = s.sync_spools(&[other]).await.unwrap();
        assert_eq!(r.missing, 1);
        let one = s.list_spools().await.unwrap().into_iter().find(|x| x.id == "1").unwrap();
        assert_eq!((one.missing, one.in_printer), (1, 0));
    }

    #[tokio::test]
    async fn rueckmeldung_vom_drucker() {
        let (s, _d) = store().await;
        let o = s.create_order(input("Figuren", None)).await.unwrap();
        let run = s
            .insert_run(NewRun { order_id: o.id.clone(), file_name: "figuren.gcode.3mf".into(), signature: "x".into(), ..Default::default() })
            .await
            .unwrap();
        let fils = vec![RunFilament { kind: "PLA".into(), color: "#DE4343".into(), grams: 41.5, spool_id: None }];
        let t = |id: &'static str, end: &'static str| PrinterTask {
            id, printer: "h2c", status: "ok", ended_at: end, actual_seconds: Some(3600), actual: &fils,
        };
        // Druck im Verlauf endete vor dem Anlegen: passt nicht
        assert!(s.record_printer_result(&o.id, None, &t("1", "2000-01-01T00:00:00Z")).await.unwrap().is_none());
        // passender Druck
        let id = s.record_printer_result(&o.id, Some("figuren.gcode.3mf"), &t("2", "2999-01-01T00:00:00Z")).await.unwrap();
        assert_eq!(id.as_deref(), Some(run.as_str()));
        let r = s.get_run(&run).await.unwrap();
        assert_eq!((r.printer_status.as_str(), r.printer_name.as_str(), r.actual_seconds), ("ok", "h2c", Some(3600)));
        assert!(r.actual_filaments.contains("41.5"));
        assert_eq!(r.result, "offen", "nur Vorschlag, Ergebnis setzt der Nutzer");
        // derselbe Verlaufseintrag wird kein zweites Mal zugeordnet
        s.insert_run(NewRun { order_id: o.id.clone(), file_name: "figuren.gcode.3mf".into(), signature: "y".into(), ..Default::default() })
            .await
            .unwrap();
        assert!(s.record_printer_result(&o.id, None, &t("2", "2999-01-01T00:00:00Z")).await.unwrap().is_none());
        assert_eq!(s.open_order_files().await.unwrap()[0].files, vec!["figuren.gcode.3mf".to_string()]);
    }

    #[tokio::test]
    async fn makerworld_sammlungen() {
        let (s, _d) = store().await;
        // per Link importiertes Projekt mit derselben MakerWorld-Adresse
        s.insert_catalog("alt", NewCatalogEntry {
            title: "Baum".into(), file_name: "baum.3mf".into(),
            source_url: Some("https://makerworld.com/de/models/3338545-christmas-tree".into()), ..Default::default()
        }).await.unwrap();
        let c = |id: &str| MwCollection { id: id.into(), title: format!("S{id}"), design_cnt: 2, is_default: 0, enabled: 1, synced_at: None };
        s.save_mw_collections(&[c("1"), c("2")]).await.unwrap();
        let d = |id: &str| MwDesign { id: id.into(), title: format!("Modell {id}"), designer: "X".into(), ..Default::default() };
        assert_eq!(s.save_mw_designs("1", &[d("3338545"), d("42")]).await.unwrap(), 1, "Baum wird verknüpft, nicht verdoppelt");
        assert_eq!(s.save_mw_designs("2", &[d("42")]).await.unwrap(), 0);
        let l = s.list_catalog(false).await.unwrap();
        assert_eq!(l.len(), 2);
        let m42 = l.iter().find(|e| e.remote_id.as_deref() == Some("42")).unwrap();
        assert_eq!((m42.origin.as_str(), m42.file_name.as_str(), m42.collection_ids.len()), ("makerworld", "", 2));
        assert_eq!(l.iter().find(|e| e.id == "alt").unwrap().remote_id.as_deref(), Some("3338545"));

        // Sammlung 1 abgewählt und Modell 42 aus Sammlung 2 entfernt: Baum und 42 ohne Sammlung -> markiert
        s.set_mw_collection_enabled("1", false).await.unwrap();
        s.save_mw_designs("2", &[]).await.unwrap();
        assert_eq!(s.mark_mw_missing().await.unwrap(), 2);
        // Sammlung verschwindet in MakerWorld
        s.save_mw_collections(&[c("2")]).await.unwrap();
        assert_eq!(s.list_mw_collections().await.unwrap().len(), 1);

        s.set_catalog_file(&m42.id, "99", &NewCatalogEntry { file_name: "m.3mf".into(), filaments: "[]".into(), license: "CC".into(), ..Default::default() }).await.unwrap();
        let e = s.get_catalog(&m42.id).await.unwrap();
        assert_eq!((e.file_name.as_str(), e.license.as_str()), ("m.3mf", "CC"));
    }
}
