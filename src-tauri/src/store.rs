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

const PERSON_SQL: &str = "
    SELECT p.id, COALESCE(p.number, 0) AS number, p.name, p.email, p.phone, p.street, p.house_no, p.zip, p.city,
           p.billing_differs, p.billing_name, p.billing_street, p.billing_house_no, p.billing_zip, p.billing_city,
           p.note, p.created_at,
           (SELECT COUNT(*) FROM orders o WHERE o.person_id = p.id) AS order_count,
           (SELECT COUNT(*) FROM orders o WHERE o.person_id = p.id AND o.archived_at IS NULL
              AND o.status NOT IN ('uebergeben', 'abgelehnt')) AS open_count,
           (SELECT MAX(o.created_at) FROM orders o WHERE o.person_id = p.id) AS last_order_at
    FROM people p";

/// Adresse in einer Zeile: „Hauptstr. 5, 12345 Berlin“ (leere Teile fallen weg)
pub fn format_address(street: &str, house_no: &str, zip: &str, city: &str) -> String {
    let line1 = [street.trim(), house_no.trim()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" ");
    let line2 = [zip.trim(), city.trim()].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join(" ");
    [line1, line2].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(", ")
}

/// Kundennummer zum Anzeigen, z.B. K-0007
pub fn customer_no(n: i64) -> String {
    format!("K-{n:04}")
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

    // ---------- Kunden ----------

    pub async fn list_people(&self) -> Result<Vec<Person>> {
        Ok(sqlx::query_as::<_, Person>(&format!("{PERSON_SQL} ORDER BY LOWER(p.name)")).fetch_all(&self.pool).await?)
    }

    pub async fn get_person(&self, id: &str) -> Result<Option<Person>> {
        Ok(sqlx::query_as::<_, Person>(&format!("{PERSON_SQL} WHERE p.id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Kunden anlegen oder ändern. Neue Kunden bekommen die nächste Kundennummer.
    /// Einen zweiten Kunden mit gleichem Namen gibt es nicht (Groß-/Kleinschreibung egal).
    pub async fn save_person(&self, p: PersonInput) -> Result<String> {
        let name = p.name.trim();
        if name.is_empty() {
            return Err(AppError::Invalid("Bitte einen Namen für den Kunden angeben.".into()));
        }
        let twin: Option<(String, i64)> = sqlx::query("SELECT id, number FROM people WHERE LOWER(name) = LOWER($1)")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| (r.get("id"), r.try_get::<Option<i64>, _>("number").ok().flatten().unwrap_or(0)));
        if let Some((tid, tnr)) = &twin {
            if p.id.as_deref() != Some(tid.as_str()) {
                return Err(AppError::Invalid(format!("Den Kunden „{name}“ gibt es schon ({}).", customer_no(*tnr))));
            }
        }
        let t = |x: &str| x.trim().to_string();
        let billing = p.billing_differs;
        // ohne abweichende Rechnungsadresse werden die Rechnungsfelder geleert
        let b = |x: &str| if billing { t(x) } else { String::new() };
        if billing && (p.billing_street.trim().is_empty() || p.billing_city.trim().is_empty()) {
            return Err(AppError::Invalid("Für die Rechnungsadresse bitte mindestens Straße und Ort angeben.".into()));
        }
        let vals = [
            t(&p.email), t(&p.phone), t(&p.street), t(&p.house_no), t(&p.zip), t(&p.city),
            b(&p.billing_name), b(&p.billing_street), b(&p.billing_house_no), b(&p.billing_zip), b(&p.billing_city),
            t(&p.note),
        ];
        // alte Freitext-Adresse für ältere Ansichten mitführen
        let address = format_address(&vals[2], &vals[3], &vals[4], &vals[5]);
        const COLS: &str = "email, phone, street, house_no, zip, city, billing_name, billing_street, billing_house_no,
            billing_zip, billing_city, note";
        match p.id {
            Some(id) => {
                let mut q = sqlx::query(
                    "UPDATE people SET name = $1, email = $2, phone = $3, street = $4, house_no = $5, zip = $6, city = $7,
                        billing_name = $8, billing_street = $9, billing_house_no = $10, billing_zip = $11, billing_city = $12,
                        note = $13, billing_differs = $14, address = $15 WHERE id = $16",
                )
                .bind(name);
                for v in &vals {
                    q = q.bind(v.clone());
                }
                let r = q.bind(billing as i64).bind(&address).bind(&id).execute(&self.pool).await?;
                if r.rows_affected() == 0 {
                    return Err(AppError::NotFound("Kunde".into()));
                }
                Ok(id)
            }
            None => {
                let id = new_id();
                let mut tx = self.pool.begin().await?;
                let number: i64 = sqlx::query("SELECT COALESCE(MAX(number), 0) + 1 AS n FROM people")
                    .fetch_one(&mut *tx)
                    .await?
                    .try_get("n")?;
                let sql = format!(
                    "INSERT INTO people (id, number, name, contact, {COLS}, billing_differs, address, created_at)
                     VALUES ($1, $2, $3, '', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)"
                );
                let mut q = sqlx::query(&sql).bind(&id).bind(number).bind(name);
                for v in &vals {
                    q = q.bind(v.clone());
                }
                q.bind(billing as i64).bind(&address).bind(now()).execute(&mut *tx).await?;
                tx.commit().await?;
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
                "Der Kunde hat noch {n} Auftrag/Aufträge und kann nicht gelöscht werden."
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
        self.save_person(PersonInput { id: None, name: name.into(), ..Default::default() }).await
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
            (Some(id), _) if !id.is_empty() => {
                if self.get_person(&id).await?.is_none() {
                    return Err(AppError::NotFound("Kunde".into()));
                }
                id
            }
            (_, Some(name)) if !name.is_empty() => self.person_by_name(name).await?,
            _ => return Err(AppError::Invalid("Bitte einen Kunden angeben.".into())),
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
        .bind(u.person_id.filter(|p| !p.is_empty()).ok_or_else(|| AppError::Invalid("Ein Auftrag braucht einen Kunden.".into()))?)
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

    /// Dateianhänge offener Aufträge: (Anhang-ID, Pfad, Auftragsnummer). Für die Auftragskennung beim Start.
    pub async fn open_model_attachments(&self) -> Result<Vec<(String, String, i64)>> {
        let rows = sqlx::query(
            "SELECT a.id, a.path, o.number FROM attachments a JOIN orders o ON o.id = a.order_id
              WHERE a.kind = 'file' AND a.path IS NOT NULL AND o.archived_at IS NULL
                AND o.status NOT IN ('uebergeben', 'abgelehnt')",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| Ok((r.try_get("id")?, r.try_get("path")?, r.try_get("number")?)))
            .collect::<std::result::Result<Vec<_>, sqlx::Error>>()
            .map_err(Into::into)
    }

    /// Datei eines Anhangs umbenennen (Pfad und Name); erkannte Drucke dieser Datei ziehen mit.
    pub async fn rename_attachment(&self, id: &str, old: &str, new: &str) -> Result<()> {
        let a = self.get_attachment(id).await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE attachments SET path = $1, name = CASE WHEN name = $2 THEN $1 ELSE name END WHERE id = $3")
            .bind(new)
            .bind(old)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE print_runs SET file_name = $1 WHERE order_id = $2 AND file_name = $3")
            .bind(new)
            .bind(&a.order_id)
            .bind(old)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
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

#[allow(dead_code)] // genutzt ab Stufe A3
const LINK_COLS: &str = "id, connector, entity, external_id, local_id, etag, remote_updated_at, synced_hash, local_hash, last_synced_at";
const SPOOL_COLS: &str = "id, brand, series, material, color, color_name, initial_grams, studio_grams,
    remaining_grams, studio_status, hidden, missing, note, updated_at, source, source_updated_at, in_printer,
    printer_name, ams_slot, depleted, rfid";
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
                            source, source_updated_at, in_printer, printer_name, ams_slot, depleted, rfid)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 0, 0, '', $11, $12, $13, $14, $15, $16, $17, $18, $19)",
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
                    .bind(&sp.rfid)
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
                            in_printer = $13, printer_name = $14, ams_slot = $15, depleted = $16, rfid = $17 WHERE id = $10"
                    } else {
                        "UPDATE spools SET brand = $1, series = $2, material = $3, color = $4, color_name = $5,
                            initial_grams = $6, studio_grams = $7, studio_status = $8, missing = 0,
                            updated_at = $9, source = $11, source_updated_at = $12,
                            in_printer = $13, printer_name = $14, ams_slot = $15, depleted = $16, rfid = $17 WHERE id = $10"
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
                        .bind(&sp.rfid)
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

    pub async fn get_spool(&self, id: &str) -> Result<Spool> {
        sqlx::query_as::<_, Spool>(&format!("SELECT {SPOOL_COLS} FROM spools WHERE id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Spule".into()))
    }

    /// Nach erfolgreichem Schreiben ins Konto: Konto-Wert und Änderungszeit übernehmen, damit der nächste
    /// Abgleich die eigene Änderung nicht für eine fremde hält.
    pub async fn spool_written(&self, id: &str, grams: f64, source_updated_at: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE spools SET studio_grams = $1, remaining_grams = $1, source_updated_at = $2, updated_at = $3 WHERE id = $4")
            .bind(grams)
            .bind(source_updated_at)
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

}

/// Anbindungen (docs/PLAN-Anbindungen.md): Zustand und Verknüpfungen.
#[allow(dead_code)] // Gerüst aus Stufe A1, genutzt ab Stufe A3
impl Store {
    /// Zustandswert einer Anbindung lesen (z.B. Sync-Token).
    pub async fn connector_state(&self, connector: &str, key: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar::<_, String>("SELECT value FROM connector_state WHERE connector = $1 AND key = $2")
            .bind(connector)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Zustandswert setzen; `None` löscht ihn.
    pub async fn set_connector_state(&self, connector: &str, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => {
                sqlx::query(
                    "INSERT INTO connector_state (connector, key, value, updated_at) VALUES ($1, $2, $3, $4)
                     ON CONFLICT (connector, key) DO UPDATE SET value = $3, updated_at = $4",
                )
                .bind(connector)
                .bind(key)
                .bind(v)
                .bind(now())
                .execute(&self.pool)
                .await?;
            }
            None => {
                sqlx::query("DELETE FROM connector_state WHERE connector = $1 AND key = $2")
                    .bind(connector)
                    .bind(key)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Alle Zustandswerte einer Anbindung löschen (beim Trennen).
    pub async fn clear_connector_state(&self, connector: &str) -> Result<()> {
        sqlx::query("DELETE FROM connector_state WHERE connector = $1").bind(connector).execute(&self.pool).await?;
        Ok(())
    }

    /// Verknüpfung anlegen oder aktualisieren; Schlüssel ist (connector, entity, external_id).
    pub async fn upsert_link(&self, l: &LinkInput) -> Result<ExternalLink> {
        sqlx::query(
            "INSERT INTO external_links (id, connector, entity, external_id, local_id, etag, remote_updated_at, synced_hash, local_hash, last_synced_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (connector, entity, external_id) DO UPDATE SET local_id = $5, etag = $6, remote_updated_at = $7,
                 synced_hash = $8, local_hash = $9, last_synced_at = $10",
        )
        .bind(new_id())
        .bind(&l.connector)
        .bind(&l.entity)
        .bind(&l.external_id)
        .bind(&l.local_id)
        .bind(&l.etag)
        .bind(&l.remote_updated_at)
        .bind(&l.synced_hash)
        .bind(&l.local_hash)
        .bind(now())
        .execute(&self.pool)
        .await?;
        self.link_by_external(&l.connector, &l.entity, &l.external_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Verknüpfung".into()))
    }

    pub async fn link_by_external(&self, connector: &str, entity: &str, external_id: &str) -> Result<Option<ExternalLink>> {
        Ok(sqlx::query_as::<_, ExternalLink>(&format!(
            "SELECT {LINK_COLS} FROM external_links WHERE connector = $1 AND entity = $2 AND external_id = $3"
        ))
        .bind(connector)
        .bind(entity)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Alle Verknüpfungen eines lokalen Datensatzes (z.B. ein Kunde mit Google und Apple).
    pub async fn links_for_local(&self, entity: &str, local_id: &str) -> Result<Vec<ExternalLink>> {
        Ok(sqlx::query_as::<_, ExternalLink>(&format!(
            "SELECT {LINK_COLS} FROM external_links WHERE entity = $1 AND local_id = $2 ORDER BY connector"
        ))
        .bind(entity)
        .bind(local_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn links_for_connector(&self, connector: &str, entity: &str) -> Result<Vec<ExternalLink>> {
        Ok(sqlx::query_as::<_, ExternalLink>(&format!(
            "SELECT {LINK_COLS} FROM external_links WHERE connector = $1 AND entity = $2 ORDER BY external_id"
        ))
        .bind(connector)
        .bind(entity)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn delete_link(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM external_links WHERE id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }

}

impl Store {
    #[allow(clippy::too_many_arguments)]
    pub async fn log_filament_write(
        &self, spool_id: &str, label: &str, action: &str, old: Option<String>, new: Option<String>, status: &str, message: &str,
        snapshot: Option<String>,
    ) -> Result<String> {
        let id = new_id();
        sqlx::query(
            "INSERT INTO filament_writes (id, created_at, spool_id, label, action, old_value, new_value, status, message, snapshot)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(&id)
        .bind(now())
        .bind(spool_id)
        .bind(label)
        .bind(action)
        .bind(old)
        .bind(new)
        .bind(status)
        .bind(message)
        .bind(snapshot)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_filament_writes(&self, limit: i64) -> Result<Vec<FilamentWrite>> {
        Ok(sqlx::query_as::<_, FilamentWrite>(&format!(
            "SELECT id, created_at, spool_id, label, action, old_value, new_value, status, message
             FROM filament_writes ORDER BY created_at DESC LIMIT {}",
            limit.clamp(1, 500)
        ))
        .fetch_all(&self.pool)
        .await?)
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
            "SELECT r.created_at,
                    CASE WHEN r.result <> 'offen' THEN r.result WHEN r.printer_status <> '' THEN r.printer_status ELSE 'offen' END AS result,
                    COALESCE(r.actual_seconds, r.print_seconds) AS print_seconds,
                    CASE WHEN r.actual_filaments <> '[]' THEN r.actual_filaments ELSE r.filaments END AS filaments,
                    o.number AS order_number,
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
        let history = self.list_history().await?;
        Ok(Stats { runs, handovers, history })
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
        const OPEN: &str = "o.archived_at IS NULL AND o.status NOT IN ('uebergeben', 'abgelehnt')";
        let mut out: Vec<OrderFiles> = Vec::new();
        for r in sqlx::query(&format!("SELECT o.id, o.number, o.title, o.status, o.created_at FROM orders o WHERE {OPEN} ORDER BY o.number"))
            .fetch_all(&self.pool)
            .await?
        {
            out.push(OrderFiles {
                order_id: r.try_get("id")?,
                number: r.try_get("number")?,
                title: r.try_get("title")?,
                status: r.try_get("status")?,
                files: Vec::new(),
                last_run_at: None,
                created_at: r.try_get("created_at")?,
            });
        }
        let files = sqlx::query(&format!(
            "SELECT a.order_id, a.name AS file, NULL AS run_at FROM attachments a JOIN orders o ON o.id = a.order_id
              WHERE a.kind = 'file' AND {OPEN}
             UNION ALL
             SELECT r.order_id, r.file_name AS file, r.created_at AS run_at FROM print_runs r JOIN orders o ON o.id = r.order_id
              WHERE {OPEN}"
        ))
        .fetch_all(&self.pool)
        .await?;
        for r in files {
            let id: String = r.try_get("order_id")?;
            let file: String = r.try_get("file")?;
            let run_at: Option<String> = r.try_get("run_at")?;
            if let Some(o) = out.iter_mut().find(|o| o.order_id == id) {
                if !o.files.contains(&file) {
                    o.files.push(file);
                }
                if run_at > o.last_run_at {
                    o.last_run_at = run_at;
                }
            }
        }
        Ok(out)
    }

    /// Von Hand festgelegter Auftrag für einen Druckauftrag (Schlüssel: vereinheitlichter Name).
    pub async fn print_assignment(&self, task_key: &str) -> Result<Option<String>> {
        Ok(sqlx::query("SELECT order_id FROM print_assignments WHERE task_key = $1")
            .bind(task_key)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.get("order_id")))
    }

    /// Zuordnung setzen (`Some`) oder aufheben (`None`, dann wieder automatisch).
    pub async fn set_print_assignment(&self, task_key: &str, order_id: Option<&str>) -> Result<()> {
        if let Some(id) = order_id {
            self.get_order(id).await?;
        }
        sqlx::query("DELETE FROM print_assignments WHERE task_key = $1").bind(task_key).execute(&self.pool).await?;
        if let Some(id) = order_id {
            sqlx::query("INSERT INTO print_assignments (task_key, order_id, created_at) VALUES ($1, $2, $3)")
                .bind(task_key)
                .bind(id)
                .bind(now())
                .execute(&self.pool)
                .await?;
        }
        Ok(())
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

// ---------- Druckverlauf (Bambu-Konto) ----------

const HISTORY_SQL: &str = "
    SELECT h.id, h.title, h.design_title, h.design_id, h.device_name, h.device_model, h.plate_index, h.status,
           h.start_time, h.end_time, h.planned_seconds, h.weight_grams, h.filaments, h.cover_url, h.order_id,
           h.matched_by, h.progress_at_end, h.used_factor, h.used_method,
           o.number AS order_number, o.title AS order_title, p.name AS person_name
    FROM print_history h LEFT JOIN orders o ON o.id = h.order_id LEFT JOIN people p ON p.id = o.person_id";

impl Store {
    pub async fn list_history(&self) -> Result<Vec<HistoryEntry>> {
        Ok(sqlx::query_as::<_, HistoryEntry>(&format!("{HISTORY_SQL} ORDER BY COALESCE(h.end_time, h.start_time) DESC"))
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn get_history(&self, id: &str) -> Result<HistoryEntry> {
        sqlx::query_as::<_, HistoryEntry>(&format!("{HISTORY_SQL} WHERE h.id = $1"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Druck im Verlauf".into()))
    }

    /// Einträge aus dem Konto übernehmen. Neue werden angelegt, bekannte aktualisiert;
    /// eine Zuordnung zum Auftrag bleibt erhalten. Gibt die Zahl neuer Einträge zurück.
    pub async fn upsert_history(&self, list: &[HistoryEntry]) -> Result<i64> {
        let ts = now();
        let mut added = 0;
        let mut tx = self.pool.begin().await?;
        for h in list {
            let exists = sqlx::query("SELECT id FROM print_history WHERE id = $1").bind(&h.id).fetch_optional(&mut *tx).await?.is_some();
            let sql = if exists {
                "UPDATE print_history SET title = $2, design_title = $3, design_id = $4, device_name = $5, device_model = $6,
                    plate_index = $7, status = $8, start_time = $9, end_time = $10, planned_seconds = $11, weight_grams = $12,
                    filaments = $13, cover_url = $14, synced_at = $15 WHERE id = $1"
            } else {
                added += 1;
                "INSERT INTO print_history (id, title, design_title, design_id, device_name, device_model, plate_index, status,
                    start_time, end_time, planned_seconds, weight_grams, filaments, cover_url, synced_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)"
            };
            sqlx::query(sql)
                .bind(&h.id)
                .bind(&h.title)
                .bind(&h.design_title)
                .bind(&h.design_id)
                .bind(&h.device_name)
                .bind(&h.device_model)
                .bind(h.plate_index)
                .bind(h.status)
                .bind(&h.start_time)
                .bind(&h.end_time)
                .bind(h.planned_seconds)
                .bind(h.weight_grams)
                .bind(&h.filaments)
                .bind(&h.cover_url)
                .bind(&ts)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(added)
    }

    /// Fortschritt eines laufenden Drucks merken (je Druckauftrag und Drucker der zuletzt gesehene Wert).
    pub async fn save_progress(&self, task_key: &str, device: &str, progress: i64) -> Result<()> {
        sqlx::query("DELETE FROM print_progress WHERE task_key = $1 AND device = $2").bind(task_key).bind(device).execute(&self.pool).await?;
        sqlx::query("INSERT INTO print_progress (task_key, device, progress, seen_at) VALUES ($1, $2, $3, $4)")
            .bind(task_key)
            .bind(device)
            .bind(progress)
            .bind(now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Zuletzt gesehener Fortschritt, sofern er zwischen Start und Ende (+10 Min.) des Drucks erfasst wurde.
    pub async fn progress_for(&self, task_key: &str, device: &str, start: &str, end: &str) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT progress, seen_at FROM print_progress WHERE task_key = $1 AND device = $2")
            .bind(task_key)
            .bind(device)
            .fetch_optional(&self.pool)
            .await?;
        let Some(r) = row else { return Ok(None) };
        let seen: String = r.try_get("seen_at")?;
        let parse = |x: &str| chrono::DateTime::parse_from_rfc3339(x).ok().map(|d| d.to_utc());
        let (Some(seen), Some(start), Some(end)) = (parse(&seen), parse(start), parse(end)) else { return Ok(None) };
        Ok((seen >= start && seen <= end + chrono::Duration::minutes(10)).then(|| r.get("progress")))
    }

    /// Geschätzten Verbrauchsanteil eines Eintrags speichern.
    pub async fn set_history_usage(&self, id: &str, factor: f64, method: &str, progress: Option<i64>) -> Result<()> {
        sqlx::query("UPDATE print_history SET used_factor = $1, used_method = $2, progress_at_end = $3 WHERE id = $4")
            .bind(factor)
            .bind(method)
            .bind(progress)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Automatische Zuordnung merken (nur für Einträge ohne Zuordnung).
    pub async fn set_history_auto(&self, id: &str, order_id: &str, by: &str) -> Result<bool> {
        let r = sqlx::query("UPDATE print_history SET order_id = $1, matched_by = $2 WHERE id = $3 AND order_id IS NULL")
            .bind(order_id)
            .bind(by)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Druck aus dem Verlauf von Hand einem Auftrag zuordnen. Dauer und Gramm kommen an einen Druck im Auftrag:
    /// an den neuesten ohne Rückmeldung oder, wenn es keinen gibt, an einen neuen Druck aus dem Verlauf.
    /// `None` hebt die Zuordnung auf (ein dafür angelegter Druck wird entfernt, sofern nichts abgezogen ist).
    pub async fn assign_history(&self, id: &str, order_id: Option<&str>) -> Result<()> {
        let h = self.get_history(id).await?;
        // alte Verknüpfung lösen
        if let Some(old) = &h.order_id {
            sqlx::query("DELETE FROM print_runs WHERE order_id = $1 AND signature = $2 AND deducted = 0")
                .bind(old)
                .bind(format!("verlauf:{id}"))
                .execute(&self.pool)
                .await?;
            sqlx::query(
                "UPDATE print_runs SET printer_name = '', printer_status = '', printer_finished_at = NULL, actual_seconds = NULL,
                    actual_filaments = '[]', printer_task_id = NULL WHERE order_id = $1 AND printer_task_id = $2",
            )
            .bind(old)
            .bind(id)
            .execute(&self.pool)
            .await?;
        }
        let Some(order_id) = order_id else {
            sqlx::query("UPDATE print_history SET order_id = NULL, matched_by = NULL WHERE id = $1").bind(id).execute(&self.pool).await?;
            return Ok(());
        };
        self.get_order(order_id).await?;
        sqlx::query("UPDATE print_history SET order_id = $1, matched_by = 'manuell' WHERE id = $2")
            .bind(order_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        let status = match h.status {
            2 => "ok",
            3 => "fehldruck",
            _ => "",
        };
        let seconds = match (h.start_time.as_deref(), h.end_time.as_deref()) {
            (Some(a), Some(b)) => match (chrono::DateTime::parse_from_rfc3339(a), chrono::DateTime::parse_from_rfc3339(b)) {
                (Ok(a), Ok(b)) if b > a => Some((b - a).num_seconds()),
                _ => None,
            },
            _ => None,
        };
        let ts = now();
        // abgebrochen: nur den geschätzt tatsächlich verbrauchten Anteil übernehmen
        let used = crate::bambu::history::scale_filaments(&h.filaments, h.used_factor.unwrap_or(1.0));
        let runs = self.list_runs(order_id).await?;
        let run_id = match runs.iter().find(|r| r.printer_status.is_empty()) {
            Some(r) => r.id.clone(),
            None => {
                let rid = new_id();
                sqlx::query(
                    "INSERT INTO print_runs (id, order_id, file_name, plates, print_seconds, weight_grams, filaments, signature,
                        result, deducted, note, created_at, updated_at)
                     VALUES ($1, $2, $3, 1, $4, $5, $6, $7, 'offen', 0, '', $8, $9)",
                )
                .bind(&rid)
                .bind(order_id)
                .bind(if h.title.is_empty() { "Druck aus dem Verlauf" } else { h.title.as_str() })
                .bind(h.planned_seconds)
                .bind(h.weight_grams)
                .bind(&h.filaments)
                .bind(format!("verlauf:{id}"))
                .bind(h.start_time.clone().unwrap_or_else(|| ts.clone()))
                .bind(&ts)
                .execute(&self.pool)
                .await?;
                rid
            }
        };
        sqlx::query(
            "UPDATE print_runs SET printer_name = $1, printer_status = $2, printer_finished_at = $3, actual_seconds = $4,
                actual_filaments = $5, printer_task_id = $6, updated_at = $7 WHERE id = $8",
        )
        .bind(&h.device_name)
        .bind(status)
        .bind(&h.end_time)
        .bind(seconds)
        .bind(&used)
        .bind(id)
        .bind(&ts)
        .bind(&run_id)
        .execute(&self.pool)
        .await?;
        let grams: f64 = serde_json::from_str::<Vec<RunFilament>>(&used).unwrap_or_default().iter().map(|f| f.grams).sum();
        self.add_event(order_id, "print", &format!("Druck aus dem Verlauf zugeordnet: {} ({})", h.title, fmt_grams(grams))).await
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

    /// Auftrag für Tests; ohne Angabe gehört er dem Kunden „Testkunde“.
    fn input(title: &str, person: Option<&str>) -> OrderInput {
        OrderInput {
            title: title.into(),
            description: String::new(),
            person_id: None,
            person_name: Some(person.unwrap_or("Testkunde").into()),
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
    async fn kunden_pflicht_nummer_und_dubletten() {
        let (s, _d) = store().await;
        // Auftrag ohne Kunde geht nicht
        let mut ohne = input("Vase", None);
        ohne.person_name = None;
        assert!(s.create_order(ohne.clone()).await.unwrap_err().to_string().contains("Kunden"));
        ohne.person_name = Some("   ".into());
        assert!(s.create_order(ohne.clone()).await.is_err());
        ohne.person_id = Some("gibt-es-nicht".into());
        assert!(s.create_order(ohne).await.is_err());

        // Kunden bekommen fortlaufende Nummern und Kontaktdaten
        let neu = |name: &str| PersonInput {
            name: name.into(), email: "a@b.de".into(), phone: "0170 1".into(), street: "Weg".into(), house_no: "1".into(),
            zip: "12345".into(), city: "Berlin".into(), ..Default::default()
        };
        let a = s.save_person(neu("Anna")).await.unwrap();
        let b = s.save_person(neu("Ben")).await.unwrap();
        let pa = s.get_person(&a).await.unwrap().unwrap();
        let pb = s.get_person(&b).await.unwrap().unwrap();
        assert_eq!((pa.number, pb.number), (1, 2));
        assert_eq!((pa.email.as_str(), pa.phone.as_str()), ("a@b.de", "0170 1"));
        assert_eq!((pa.street.as_str(), pa.house_no.as_str(), pa.zip.as_str(), pa.city.as_str()), ("Weg", "1", "12345", "Berlin"));
        assert_eq!(pa.billing_differs, 0);
        assert_eq!(customer_no(pb.number), "K-0002");
        // gleicher Name ein zweites Mal: Fehler mit Kundennummer
        assert!(s.save_person(neu("anna")).await.unwrap_err().to_string().contains("K-0001"));
        // eigenen Namen behalten beim Ändern geht
        s.save_person(PersonInput { id: Some(a.clone()), ..neu("Anna") }).await.unwrap();

        // Auftrag mit Kunden-ID; Kunde entfernen beim Ändern geht nicht
        let mut mit = input("Becher", None);
        mit.person_name = None;
        mit.person_id = Some(a.clone());
        let o = s.create_order(mit).await.unwrap();
        let pa = s.get_person(&a).await.unwrap().unwrap();
        assert_eq!((pa.order_count, pa.open_count), (1, 1));
        assert!(pa.last_order_at.is_some());
        let upd = OrderUpdate { id: o.id.clone(), title: "Becher".into(), description: String::new(), person_id: None, priority: 0, due_date: None };
        assert!(s.update_order(upd).await.is_err());
    }

    #[tokio::test]
    async fn status_und_verlauf() {
        let (s, d) = store().await;
        let a = s.create_order(input("A", None)).await.unwrap();
        let b = s.create_order(input("B", None)).await.unwrap();
        let list = s.list_orders(None).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|o| o.id == b.id));

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
    async fn anbindungen_zustand_und_verknuepfungen() {
        let (s, _d) = store().await;
        // Zustand: setzen, überschreiben, löschen
        assert_eq!(s.connector_state("google-contacts", "sync_token").await.unwrap(), None);
        s.set_connector_state("google-contacts", "sync_token", Some("a")).await.unwrap();
        s.set_connector_state("google-contacts", "sync_token", Some("b")).await.unwrap();
        s.set_connector_state("carddav", "sync_token", Some("x")).await.unwrap();
        assert_eq!(s.connector_state("google-contacts", "sync_token").await.unwrap().as_deref(), Some("b"));
        s.set_connector_state("google-contacts", "sync_token", None).await.unwrap();
        assert_eq!(s.connector_state("google-contacts", "sync_token").await.unwrap(), None);
        s.clear_connector_state("carddav").await.unwrap();
        assert_eq!(s.connector_state("carddav", "sync_token").await.unwrap(), None);

        // Verknüpfung: anlegen, aktualisieren (gleicher Schlüssel), zweite Anbindung am selben Kunden
        let mut l = LinkInput {
            connector: "google-contacts".into(), entity: "person".into(), external_id: "people/c1".into(),
            local_id: "p1".into(), etag: "e1".into(), synced_hash: "h1".into(), ..Default::default()
        };
        let first = s.upsert_link(&l).await.unwrap();
        l.etag = "e2".into();
        l.synced_hash = "h2".into();
        let second = s.upsert_link(&l).await.unwrap();
        assert_eq!(first.id, second.id, "gleicher Schlüssel → gleiche Verknüpfung");
        assert_eq!((second.etag.as_str(), second.synced_hash.as_str()), ("e2", "h2"));
        assert!(second.last_synced_at.is_some());
        s.upsert_link(&LinkInput { connector: "carddav".into(), entity: "person".into(), external_id: "uid-1".into(), local_id: "p1".into(), ..Default::default() })
            .await
            .unwrap();
        let all = s.links_for_local("person", "p1").await.unwrap();
        assert_eq!(all.iter().map(|x| x.connector.as_str()).collect::<Vec<_>>(), ["carddav", "google-contacts"]);
        assert_eq!(s.links_for_connector("google-contacts", "person").await.unwrap().len(), 1);
        s.delete_link(&first.id).await.unwrap();
        assert!(s.link_by_external("google-contacts", "person", "people/c1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rest_ins_konto_geschrieben() {
        let (s, _d) = store().await;
        let k = StudioSpool {
            id: "7".into(), material: "PLA".into(), initial_grams: 1000.0, net_grams: 650.0, source: "konto".into(),
            source_updated_at: Some("2026-11-16T08:27:03Z".into()), ..Default::default()
        };
        s.sync_spools(std::slice::from_ref(&k)).await.unwrap();
        // in der App abgezogen, dann ins Konto geschrieben
        s.spool_written("7", 588.0, Some("2026-11-17T10:00:00Z")).await.unwrap();
        let sp = s.get_spool("7").await.unwrap();
        assert_eq!((sp.studio_grams, sp.remaining_grams, sp.source_updated_at.as_deref()), (588.0, 588.0, Some("2026-11-17T10:00:00Z")));
        // nächster Abgleich liefert denselben Stand: bleibt 588 g
        let mut same = k.clone();
        same.net_grams = 588.0;
        same.source_updated_at = Some("2026-11-17T10:00:00Z".into());
        s.sync_spools(&[same]).await.unwrap();
        assert_eq!(s.get_spool("7").await.unwrap().remaining_grams, 588.0);

        // Protokoll: neuester Eintrag zuerst
        s.log_filament_write("7", "PLA", "rest", Some("650".into()), Some("588".into()), "ok", "", None).await.unwrap();
        s.log_filament_write("7", "PLA", "rest", Some("588".into()), Some("550".into()), "wartet", "", None).await.unwrap();
        let w = s.list_filament_writes(10).await.unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!((w[0].status.as_str(), w[0].new_value.as_deref()), ("wartet", Some("550")));
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

    #[tokio::test]
    async fn adresse_und_rechnungsadresse() {
        let (s, _d) = store().await;
        assert_eq!(format_address("Hauptstr.", "5a", "12345", "Berlin"), "Hauptstr. 5a, 12345 Berlin");
        assert_eq!(format_address("", "", "12345", "Berlin"), "12345 Berlin");
        assert_eq!(format_address(" ", "", "", ""), "");

        let mut k = PersonInput {
            name: "Firma Holz".into(), street: "Werkweg".into(), house_no: "3".into(), zip: "70173".into(), city: "Stuttgart".into(),
            billing_differs: true, billing_name: "Holz GmbH".into(), billing_street: "Postfach".into(), billing_zip: "70001".into(),
            billing_city: "Stuttgart".into(), ..Default::default()
        };
        let id = s.save_person(k.clone()).await.unwrap();
        let p = s.get_person(&id).await.unwrap().unwrap();
        assert_eq!((p.billing_differs, p.billing_name.as_str(), p.billing_street.as_str()), (1, "Holz GmbH", "Postfach"));

        // Rechnungsadresse ohne Straße/Ort wird abgelehnt
        k.id = Some(id.clone());
        k.billing_city = " ".into();
        assert!(s.save_person(k.clone()).await.is_err());

        // ohne abweichende Rechnungsadresse werden die Felder geleert
        k.billing_differs = false;
        s.save_person(k).await.unwrap();
        let p = s.get_person(&id).await.unwrap().unwrap();
        assert_eq!((p.billing_differs, p.billing_name.as_str(), p.billing_street.as_str()), (0, "", ""));
        assert_eq!(p.street, "Werkweg");
    }

    #[tokio::test]
    async fn umbenennen_und_feste_zuordnung() {
        let (s, _d) = store().await;
        let o = s.create_order(input("Benchy", None)).await.unwrap();
        let att = s.add_attachment(&o.id, "file", "3dbenchy.stl", Some("3dbenchy.stl"), None, None).await.unwrap();
        s.insert_run(NewRun { order_id: o.id.clone(), file_name: "3dbenchy.stl".into(), signature: "x".into(), ..Default::default() })
            .await
            .unwrap();
        s.rename_attachment(&att, "3dbenchy.stl", "A0001 3dbenchy.stl").await.unwrap();
        let a = s.get_attachment(&att).await.unwrap();
        assert_eq!((a.name.as_str(), a.path.as_deref()), ("A0001 3dbenchy.stl", Some("A0001 3dbenchy.stl")));
        assert_eq!(s.list_runs(&o.id).await.unwrap()[0].file_name, "A0001 3dbenchy.stl", "Drucke ziehen mit");
        let f = s.open_order_files().await.unwrap();
        assert_eq!((f[0].status.as_str(), f[0].files.len()), ("eingang", 1));
        assert!(f[0].last_run_at.is_some());

        assert_eq!(s.print_assignment("benchy").await.unwrap(), None);
        s.set_print_assignment("benchy", Some(&o.id)).await.unwrap();
        assert_eq!(s.print_assignment("benchy").await.unwrap().as_deref(), Some(o.id.as_str()));
        assert!(s.set_print_assignment("benchy", Some("gibt-es-nicht")).await.is_err());
        s.set_print_assignment("benchy", None).await.unwrap();
        assert_eq!(s.print_assignment("benchy").await.unwrap(), None);
    }

    #[tokio::test]
    async fn druckverlauf_und_nachtraeglich_zuordnen() {
        let (s, _d) = store().await;
        let h = |id: &str, title: &str, status: i64| HistoryEntry {
            id: id.into(), title: title.into(), device_name: "h2c".into(), status,
            start_time: Some("2026-11-16T07:00:00Z".into()), end_time: Some("2026-11-16T09:00:00Z".into()),
            planned_seconds: Some(7000), weight_grams: Some(42.0),
            filaments: r##"[{"type":"PLA","color":"#DE4343","grams":42.0}]"##.into(), ..Default::default()
        };
        assert_eq!(s.upsert_history(&[h("1", "benchy_plate_1", 2), h("2", "vase", 3)]).await.unwrap(), 2);
        // erneut: nichts neu, Titel aktualisiert
        assert_eq!(s.upsert_history(&[h("1", "benchy_plate_1b", 2)]).await.unwrap(), 0);
        assert_eq!(s.get_history("1").await.unwrap().title, "benchy_plate_1b");

        // Auftrag ohne Druck: Zuordnen legt einen Druck mit echter Dauer und Gramm an
        let o = s.create_order(input("Benchy", None)).await.unwrap();
        s.assign_history("1", Some(&o.id)).await.unwrap();
        let runs = s.list_runs(&o.id).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!((runs[0].printer_status.as_str(), runs[0].actual_seconds), ("ok", Some(7200)));
        assert!(runs[0].actual_filaments.contains("42"));
        let e = s.get_history("1").await.unwrap();
        assert_eq!((e.order_number, e.matched_by.as_deref()), (Some(o.number), Some("manuell")));
        // automatische Zuordnung überschreibt keine bestehende
        assert!(!s.set_history_auto("1", &o.id, "datei").await.unwrap());

        // Zuordnung aufheben: der dafür angelegte Druck verschwindet wieder
        s.assign_history("1", None).await.unwrap();
        assert!(s.list_runs(&o.id).await.unwrap().is_empty());
        assert!(s.get_history("1").await.unwrap().order_id.is_none());

        // Auftrag mit offenem Druck: Rückmeldung kommt an diesen Druck, abgebrochen = Fehldruck
        s.insert_run(NewRun { order_id: o.id.clone(), file_name: "vase.3mf".into(), signature: "x".into(), ..Default::default() })
            .await
            .unwrap();
        s.assign_history("2", Some(&o.id)).await.unwrap();
        let runs = s.list_runs(&o.id).await.unwrap();
        assert_eq!((runs.len(), runs[0].printer_status.as_str()), (1, "fehldruck"));
        assert_eq!(s.stats().await.unwrap().history.len(), 2);
    }

    #[tokio::test]
    async fn fortschritt_merken() {
        let (s, _d) = store().await;
        s.save_progress("vase", "h2c", 30).await.unwrap();
        s.save_progress("vase", "h2c", 42).await.unwrap();
        let start = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let end = chrono::Utc::now().to_rfc3339();
        assert_eq!(s.progress_for("vase", "h2c", &start, &end).await.unwrap(), Some(42), "letzter Wert zählt");
        assert_eq!(s.progress_for("vase", "a1", &start, &end).await.unwrap(), None, "anderer Drucker");
        // gesehen außerhalb des Druckzeitraums: gehört zu einem anderen Druck
        let old_end = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        let old_start = (chrono::Utc::now() - chrono::Duration::hours(3)).to_rfc3339();
        assert_eq!(s.progress_for("vase", "h2c", &old_start, &old_end).await.unwrap(), None);
    }

    /// Prüfung mit echten Daten auf einer KOPIE der Datenbank (nie die Originaldatei angeben):
    /// `PM_DB_COPY=/pfad/kopie.db cargo test echte_kopie -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn echte_kopie_verlauf_zuordnen() {
        let Ok(path) = std::env::var("PM_DB_COPY") else { return };
        assert!(!path.contains("Application Support"), "nur eine Kopie verwenden");
        let s = Store::connect(&Store::sqlite_url(std::path::Path::new(&path))).await.unwrap();
        let history = s.list_history().await.unwrap();
        let h = history.iter().find(|h| h.status == 2 && h.order_id.is_none()).expect("fertiger Druck im Verlauf");
        let o = s.list_orders(None).await.unwrap().into_iter().find(|o| o.archived_at.is_none()).expect("offener Auftrag");
        let before = s.list_runs(&o.id).await.unwrap().len();
        s.assign_history(&h.id, Some(&o.id)).await.unwrap();
        let runs = s.list_runs(&o.id).await.unwrap();
        let r = runs.iter().find(|r| r.printer_finished_at == h.end_time && !r.printer_status.is_empty()).expect("Druck mit Rückmeldung");
        println!(
            "Verlauf {} „{}“ → #{}: Druck {} · Status {} · {} s · {} · Drucke vorher {before}, nachher {}",
            h.id, h.title, o.number, r.file_name, r.printer_status, r.actual_seconds.unwrap_or(0), r.actual_filaments, runs.len()
        );
        assert_eq!(r.printer_status, "ok");
        assert!(r.actual_seconds.unwrap_or(0) > 0);
        assert_eq!(s.get_history(&h.id).await.unwrap().order_number, Some(o.number));
        s.assign_history(&h.id, None).await.unwrap();
        assert_eq!(s.list_runs(&o.id).await.unwrap().len(), before, "Aufheben stellt den alten Stand her");
    }
}
