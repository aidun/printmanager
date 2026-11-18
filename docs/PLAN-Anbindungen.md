# Plan: Externe Anbindungen entkoppeln – danach Kontakte

Stand 18.11.2026. Entscheidungen vom 18.11.2026 sind eingearbeitet (Abschnitt 6). **A1 (Gerüst), A2 (Fachlogik in den Kern), A3 (Bambu als Anbindung) und A4 (Web, Printables, Link-Resolver) sind umgesetzt**, offen sind A5, A6 und die Kontakte.

## Ziel

Alles, was Printmanager mit der Außenwelt verbindet, soll über **eine** einheitliche Schicht laufen. Dazu gehören:
- das Bambu-Konto
- Bambu Studio
- MakerWorld und Printables
- später andere Slicer, Druckdienste, Kontakte und Speicher

Eine neue Anbindung soll sich wie die vorhandenen einfügen:
- gleiche Anmeldung
- gleiche Einstellungen
- gleiche Synchronisation
- gleiches Protokoll
- gleiche Sicherheitsregeln

Der Kern der App (Aufträge, Kunden, Spulen, Drucke, Katalog) kennt dann keinen Anbieter mehr, sondern nur noch **Fähigkeiten** wie „liefert Kontakte“ oder „liefert Spulen“.

Zuerst ziehen die vorhandenen Anbindungen (Bambu und Co.) ohne sichtbare Änderung in die neue Schicht um. Danach kommen als erste neue Anbindungen **Google Kontakte** und **Apple Kontakte**, nur lesend.

## Begriffe

| Begriff | Bedeutung | Beispiel |
|---|---|---|
| **Anbindung** (Connector) | ein externer Dienst oder ein Programm, mit Anmeldung, Einstellungen und Status | Bambu-Konto, Google Kontakte, Bambu Studio |
| **Fähigkeit** (Capability) | eine fachliche Schnittstelle, die eine Anbindung erfüllen kann, getrennt nach Lesen und Schreiben | Kontakte lesen, Spulen schreiben, Datei im Slicer öffnen |
| **Adapter** | der Code, der eine Fähigkeit für einen Anbieter umsetzt | `google_contacts`, `bambu_cloud` |
| **Verknüpfung** | Zuordnung eines lokalen Datensatzes zu einem externen | Kunde K-0003 ↔ Google-Kontakt `people/c123` |

---

## 1. Bestand: alle heutigen Außenschnittstellen

| Anbindung | Was | Wo im Code | Art | Kopplung heute |
|---|---|---|---|---|
| Bambu-Konto: Anmeldung | Web-Anmeldung, Token aus Cookie | `bambu/mod.rs` (`bambu_login`, `entry()`) | Webview + Schlüsselbund | eigener Schlüsselbund-Eintrag, eigenes Fenster, eigener Status |
| Bambu-Konto: Filament | Spulen lesen, Rest/Anlegen/Löschen schreiben | `bambu/mod.rs` (`sync_filament`), `bambu/filament_write.rs`, `filament.rs` (`parse_cloud`) | HTTP JSON | Quelle als Text `konto`, Protokoll `filament_writes` nur für Bambu |
| Bambu-Konto: Drucker | Liste, Status, laufender Druck | `bambu/printers.rs` (`poll`) | HTTP JSON | Zuordnung zu Aufträgen (`find_order`) steckt im Bambu-Modul |
| Bambu-Konto: Druckverlauf | Aufgaben, Gramm, Dauer | `bambu/history.rs` | HTTP JSON | Tabelle `print_history` mit Bambu-Feldern |
| MakerWorld | Sammlungen, Modelle, 3MF bei Bedarf | `bambu/makerworld.rs` | HTTP JSON + CDN | eigener `reqwest::Client` ohne Drosselung |
| Link-Import | Printables (GraphQL), MakerWorld, Thingiverse, Web-Vorschau | `linkimport.rs` | HTTP | eigener Client, Erkennung per Regex |
| Bambu Studio: Start | Datei öffnen | `commands.rs` (`studio_path`, `launch_studio`) | Prozess | Pfade fest im Code, nur Bambu Studio |
| Bambu Studio: Filament | `spools.json` lesen | `filament.rs` (`parse`, `read`), `watch.rs` | Datei | Quelle als Text `datei` |
| Bambu Studio: Ergebnis | gesliced 3MF im Auftragsordner → Druck | `watch.rs` (`handle_file`), `threemf.rs` | Datei + Format | 3MF-Lesen auf Bambu-Metadaten zugeschnitten |
| Schlüsselbund | Token | `bambu/mod.rs` | OS | Dienst und Konto als Konstanten im Bambu-Modul |
| Takt | Abgleiche alle 20 s | `bambu::start` | Hintergrund | ein fester Ablauf nur für Bambu |
| Oberfläche | Schalter, Status, Protokoll | `SettingsView.tsx` (`BambuAccount`, `MwCollections`, `WriteLog`), `api.ts` | UI | je Dienst eigener Abschnitt und eigene Befehle |

**Was das Entkoppeln heute verhindert:**
1. Der zentrale Zustand `Bambu` vermischt Anmeldung, Takt, Drucker-Cache und Konfiguration.
2. Fachlogik steckt im Adapter. Die Zuordnung „Druck → Auftrag“ (`find_order`) und die Verbrauchsschätzung (`usage_factor`) liegen in `bambu/`, gehören aber zum Kern.
3. Herkunft wird über freie Texte (`datei`, `konto`) und Bambu-Felder (`ams_slot`, `rfid`) unterschieden statt über eine Anbindungs-ID.
4. HTTP-Clients, Drosselung, Schreibschutz (`write_allowed`) und Schlüsselbund sind mehrfach oder nur für Bambu umgesetzt.
5. Jede neue Anbindung bräuchte eigene Tauri-Befehle, Einstellungen und Oberfläche.

---

## 2. Zielarchitektur

```
 Oberfläche (React)
   Einstellungen → „Anbindungen“ (generisch)   Fachansichten (Kunden, Filament, Drucker …)
          │ connector_*-Befehle                          │ Fachbefehle (unverändert)
 ─────────┼──────────────────────────────────────────────┼─────────────────────────────
 Kern     │                                              │
   Registry ── Scheduler ── SyncEngine ── WriteGuard ── Fachlogik (Zuordnung, Verbrauch, Kalkulation)
          │            Fähigkeiten (Rust-Traits, neutrale Datentypen)
 ─────────┼──────────────────────────────────────────────────────────────────────────
 Adapter: bambu_cloud · bambu_studio · makerworld · printables · google_contacts · apple_contacts · carddav · (orca_slicer, prusa_connect …)
          │
 Gemeinsame Dienste: HttpGateway (Drosselung, Schreib-Allowlist, User-Agent) · SecretStore (Schlüsselbund) · AuthFlows · Protokoll
```

### 2.1 Die Anbindung (Rust)

Neues Modul `src-tauri/src/connectors/` mit:
- `mod.rs`: Registry und Befehle
- `model.rs`: neutrale Datentypen
- `auth.rs`, `http.rs`, `secrets.rs`, `sync.rs`
- je Adapter ein Untermodul

```rust
/// Beschreibung einer Anbindung – statisch, für Oberfläche und Registry.
pub struct Descriptor {
    pub id: &'static str,               // "google-contacts"
    pub name: &'static str,             // "Google Kontakte"
    pub kind: Kind,                     // Contacts | Slicer | PrinterCloud | Catalog | Storage
    pub platforms: &'static [Platform], // macOS, Windows
    pub auth: AuthKind,                 // None | WebviewCookie | OAuthPkce | AppPassword | OsPermission
    pub capabilities: &'static [Cap],   // z.B. [ContactsRead] oder [SpoolsRead, SpoolsWrite]
    pub write_paths: &'static [&'static str], // erlaubte Schreib-Pfade (verallgemeinertes write_allowed)
    pub min_interval: Duration,         // Drosselung je Anbindung
}

#[async_trait]
pub trait Connector: Send + Sync {
    fn descriptor(&self) -> &'static Descriptor;
    async fn status(&self) -> Status;             // verbunden, abgelaufen, Fehler, zuletzt erreicht
    async fn connect(&self, ctx: &ConnCtx) -> Result<()>;    // startet den passenden AuthFlow
    async fn disconnect(&self, ctx: &ConnCtx) -> Result<()>; // löscht Geheimnisse, behält Verknüpfungen
    // Zugriff auf Fähigkeiten; None = nicht unterstützt
    fn contacts(&self) -> Option<&dyn ContactSource> { None }
    fn spools(&self) -> Option<&dyn SpoolSource> { None }
    fn spool_sink(&self) -> Option<&dyn SpoolSink> { None }
    fn printers(&self) -> Option<&dyn PrinterSource> { None }
    fn print_history(&self) -> Option<&dyn PrintHistorySource> { None }
    fn catalog(&self) -> Option<&dyn CatalogSource> { None }
    fn links(&self) -> Option<&dyn ModelLinkResolver> { None }
    fn slicer(&self) -> Option<&dyn SlicerAdapter> { None }
}
```

**Fähigkeiten** sind kleine Traits mit neutralen Typen. Sie enthalten keine Bambu- oder Google-Felder, Anbieter-Daten liegen in `extra: serde_json::Value`.

| Fähigkeit | Lesen | Schreiben | Erste Umsetzer |
|---|---|---|---|
| `ContactSource` | `list(cursor) -> Page<ExtContact>`, `changes(cursor)` | – (vorerst kein Schreiben ins Adressbuch) | Google, Apple (Mac), CardDAV |
| `SpoolSource` / `SpoolSink` | `list()` | `set_rest`, `create`, `delete` | Bambu-Konto, Bambu Studio (`spools.json`, nur lesen) |
| `PrinterSource` | `list() -> Vec<ExtPrinter>`, `current_jobs()` | – (Regel 1: nie steuern) | Bambu-Konto; später Prusa Connect, Moonraker (nur lesen) |
| `PrintHistorySource` | `jobs(since)` | – | Bambu-Konto |
| `CatalogSource` | `collections()`, `items(collection)`, `fetch_file(item)` | – | MakerWorld |
| `ModelLinkResolver` | `matches(url)`, `preview(url)`, `download(file)` | – | Printables, MakerWorld, Thingiverse, Web |
| `SlicerAdapter` | `detect()`, `read_project(path) -> ProjectInfo`, `output_patterns()` | `open(path)` (Programmstart, keine Druckbefehle) | Bambu Studio; später OrcaSlicer, PrusaSlicer |

### 2.2 Gemeinsame Dienste

- **HttpGateway:** Jede Anbindung bekommt ihren Client über die Registry. Darin stecken:
  - Drosselung nach `min_interval`
  - Zeitlimit und User-Agent
  - Schreibschutz: Nicht-`GET` nur auf `write_paths` und nur bei eingeschaltetem Schreibschalter (verallgemeinert `send_raw` und `write_allowed`)
  - Logs ohne Token und ohne Personendaten

  `linkimport.rs` und `makerworld.rs` verlieren ihre eigenen Clients.
- **SecretStore:** einheitlich `keyring`, Dienst `de.printmanager.app`, Konto `<connector-id>:<name>`, z.B. `bambu-cloud:token` oder `google-contacts:refresh`. Der alte Eintrag `bambu-token` wird einmalig übernommen.
- **AuthFlows**, wiederverwendbar:
  - `WebviewCookie` (Bambu, heutiges Login-Fenster)
  - `OAuthPkce` (Google; Systembrowser plus Loopback)
  - `AppPassword` (CardDAV; Eingabe in einem eigenen Dialog, direkt in den Schlüsselbund)
  - `OsPermission` (Apple-Kontakte am Mac)
- **SyncEngine:** Sie hält Verknüpfungen, Cursor und Konfliktregel für alle lesenden Fähigkeiten. Die Regel ist die heutige Spulen-Regel, verallgemeinert:
  - Hat sich nur die Quelle geändert, wird sie übernommen.
  - Hat sich nur die App geändert, bleibt der Wert der App.
  - Haben sich beide geändert, gibt es einen **Konflikt** mit Dialog („Wert von X übernehmen“ / „App behalten“).
- **WriteGuard und Protokoll:** Das heutige `filament_writes` wird zu `connector_writes` mit Anbindung, Fähigkeit, Datensatz, alt/neu, Status und Kopie. Daran hängen „Rückgängig“ und die Liste in den Einstellungen für alle Anbindungen.
- **Scheduler:** Statt `bambu::start` ein Takt für alle. Jede Anbindung meldet je Fähigkeit ein Intervall (z.B. Drucker 60 s, beim Drucken 20 s, Kontakte 30 min). Der Scheduler ruft nur verbundene und eingeschaltete Fähigkeiten auf und meldet Änderungen wie bisher über `pm:external`.

### 2.3 Daten

Neue Migration `0014_connectors.sql`, portables SQL:

```sql
CREATE TABLE external_links (
  id TEXT PRIMARY KEY,
  connector TEXT NOT NULL,          -- "google-contacts"
  entity TEXT NOT NULL,             -- "person" | "spool" | "catalog" | "print"
  external_id TEXT NOT NULL,        -- "people/c123", vCard-UID, Bambu-ID
  local_id TEXT NOT NULL,
  etag TEXT NOT NULL DEFAULT '',
  remote_updated_at TEXT,
  synced_hash TEXT NOT NULL DEFAULT '',  -- Hash der zugeordneten Felder beim letzten Abgleich
  local_hash TEXT NOT NULL DEFAULT '',
  last_synced_at TEXT,
  UNIQUE (connector, entity, external_id)
);
CREATE INDEX external_links_local ON external_links (entity, local_id);

CREATE TABLE connector_state (connector TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY (connector, key));

CREATE TABLE connector_writes ( /* wie filament_writes, plus connector und entity */ );
```

Umgesetzt in A1 (`0014_connectors.sql`): `external_links` und `connector_state`. `connector_writes` folgt mit A3, wenn das Bambu-Protokoll umzieht.

- `spools.source` (`datei`/`konto`) wird über `external_links` mit `connector` beschrieben. Die Spalte bleibt zur Kompatibilität, bis die Bambu-Migration abgeschlossen ist.
- **Einstellungen:** `bambu.json` geht in `connectors.json` auf, mit diesem Inhalt je Anbindung:
  - `enabled`
  - Schalter je Fähigkeit (lesen/schreiben)
  - eigene Optionen, z.B. MakerWorld-Sammlungen oder eine eigene Google-Client-ID

  Die bestehende Datei wird beim ersten Start übernommen.

### 2.4 Oberfläche

- **Einstellungen → Anbindungen:** eine Liste aus `connectors_list`, je Eintrag:
  - Name, Art, Status, „zuletzt erreicht“
  - Verbinden/Trennen, passend zur Anmeldeart
  - Schalter je Fähigkeit; Schreiben bleibt ab Werk aus und hat den gleichen Bestätigungsdialog wie heute bei Filament

  Optionen der Anbindung stehen in einem aufklappbaren Bereich. Das gemeinsame Protokoll bleibt darunter.
- **Fachansichten fragen nach Fähigkeiten, nicht nach Anbietern.** Beispiele:
  - Die Kundenseite zeigt „Aus Kontakten übernehmen“, sobald irgendeine Kontaktquelle verbunden ist.
  - Der Filament-Dialog „Ins Konto schreiben“ wird zu „In *Quelle* schreiben“.
- **Neue generische Befehle:** `connectors_list`, `connector_connect`, `connector_disconnect`, `connector_set_options`, `connector_sync(id, capability)`, `connector_writes`, `connector_write_undo`.

  Die Fachbefehle (`spool_push_rest` usw.) bleiben und leiten intern an die Anbindung der Spule weiter.

### 2.5 Regeln (AGENTS.md wird angepasst)

1. Regel 1 bleibt für jede Anbindung: Drucker werden nie gesteuert, auch nicht über Prusa Connect, Moonraker oder OctoPrint.
2. Regel 2 wird allgemein:
   - Jede Anbindung liest ab Werk nur.
   - Schreiben gibt es je Anbindung und Fähigkeit nur mit eigenem Schalter, nur auf den angegebenen `write_paths` und nur nach Bestätigung.
   - Jeder Schreibvorgang wird protokolliert.
3. Geheimnisse liegen nur im Schlüsselbund (SecretStore). Kontaktdaten und Tokens gehören nie in Logs, Tests oder Commits.
4. Personendaten aus Kontakten verlassen den Rechner nur in Richtung ihres Anbieters.

---

## 3. Bambu und Co. umziehen (ohne sichtbare Änderung) – zuerst

Diese Stufen kommen **vor** den Kontakten. Reihenfolge so, dass jede Stufe für sich läuft und getestet ist:

| Stufe | Inhalt | Ergebnis |
|---|---|---|
| A1 ✅ | Gerüst: `connectors/` mit Registry, Descriptor, HttpGateway, SecretScope, Konfiguration `connectors.json`, Scheduler; Migration `0014` mit `external_links` und `connector_state`; 8 Contract-Tests mit einer Test-Anbindung, 1 Store-Test | Grundlage, noch ungenutzt (`allow(dead_code)` bis A3) |
| A2 ✅ | Fachlogik in den Kern: `find_order`, `similarity`, `normalize`, `order_number_in` nach `matching.rs`; `usage_factor`, `scale_filaments` nach `usage.rs` mit neutralem `JobOutcome` statt Bambu-Status | Adapter ohne Fachlogik; Bambu-Statusübersetzung (`state`, `outcome`) bleibt im Adapter |
| A3 ✅ | `bambu-cloud` als Anbindung (`BambuConnector`): Fähigkeiten Spulen lesen/schreiben, Drucker, Druckverlauf, Katalog; HTTP über `HttpGateway` (`cloud.rs` entfällt); `bambu.json` → `connectors.json` (alte Datei bleibt); Token wird nach `bambu-cloud:token` kopiert (alter Eintrag bleibt für ein Zurückwechseln, Abmelden löscht beide); gemeinsamer Takt `connectors::start` mit dynamischem Takt und Anmeldeprüfung. `connector_writes` folgt mit A6. An der echten App geprüft (18.11.2026): Übernahme von Schaltern, Kontoangaben und Token, Anmeldung, Drucker und Filament-Abgleich wie vorher | gleiches Verhalten, neue Hülle |
| A4 ✅ | MakerWorld bleibt Fähigkeit „Katalog“ des Bambu-Kontos (gleicher Token und API), der 3MF-Download vom CDN läuft über den Web-Zugang. Neue öffentliche Anbindungen **Web** und **Printables** (`PublicConnector`, ab Werk an). Link-Import: je Plattform ein `LinkResolver` (Printables, MakerWorld, Thingiverse, Web). `HttpGateway` kann jetzt absolute Adressen (nur ohne Basis und ohne Token), Bytes und lesende POST-Abfragen (`read_post_paths`) | eigene HTTP-Clients entfallen; Netztest grün |
| A5 | `bambu_studio` als SlicerAdapter: `detect`, `open`, `spools.json` als SpoolSource, `read_project` (heutiges `threemf.rs`) | Grundlage für OrcaSlicer/PrusaSlicer |
| A6 | Oberfläche „Anbindungen“ generisch; alte Bambu-Befehle und `BambuAccount`-Abschnitt entfernt | eine Einstellungsseite für alles |

**Neue Slicer später:**
- **OrcaSlicer** kommt mit fast gleichem 3MF und eigenem Programmpfad aus (ein Adapter mit wenigen Zeilen).
- **PrusaSlicer** braucht ein eigenes `read_project`, weil die Metadaten in G-Code-Kommentaren und `Metadata/Slic3r_PE.config` stehen.

**Cloud-Dienste später:**
- Prusa Connect und Moonraker als PrinterSource, nur lesend.
- Nextcloud als CardDAV-Kontaktquelle oder als Speicher.

---

## 4. Danach: Kontakte (nur lesen)

### 4.1 Was der Nutzer davon hat

1. **Kunde aus Kontakt anlegen:** Im Assistenten „Neuer Auftrag“ und auf der Kundenseite sucht „Aus Kontakten …“ in allen verbundenen Kontaktquellen. Ein Klick übernimmt Name, E-Mail, Telefon und Adresse und verknüpft den Kunden.
2. **Bestehenden Kunden verknüpfen:** Die App schlägt passende Kontakte vor, anhand von E-Mail, Telefon oder Name.
3. **Änderungen übernehmen:** Zieht ein verknüpfter Kontakt um oder hat eine neue Nummer, aktualisiert die App den Kunden. Bei gleichzeitiger Änderung in der App erscheint ein Konflikt.
Die App **schreibt nicht** ins Adressbuch (entschieden 18.11.2026). Es wird auch **nicht** das ganze Adressbuch in die Kundenliste kopiert. Abgeglichen werden nur verknüpfte Kontakte.

### 4.2 Neutrales Kontaktmodell und Zuordnung

```rust
pub struct ExtContact {
    pub external_id: String,     // Google resourceName | CNContact.identifier | vCard UID
    pub etag: String,
    pub updated_at: Option<String>,
    pub display_name: String, pub given_name: String, pub family_name: String,
    pub organization: String,
    pub emails: Vec<Typed<String>>,     // Typ: privat, arbeit, sonstige; primär markiert
    pub phones: Vec<Typed<String>>,
    pub addresses: Vec<Typed<PostalAddress>>, // street (inkl. Hausnr.), zip, city, country
    pub deleted: bool,
}
```

| Kunde (`people`) | aus Kontakt |
|---|---|
| `name` | Anzeigename, sonst „Vorname Nachname“, sonst Firma |
| `email`, `phone` | primärer Eintrag, sonst der erste (Mobil vor Festnetz) |
| `street` + `house_no` | aus `street` getrennt (z.B. „Musterstraße 12a“ → „Musterstraße“ + „12a“; deutsche und einfache internationale Muster, sonst alles in `street`) |
| `zip`, `city` | direkt |
| Rechnungsadresse | nur wenn der Nutzer beim Übernehmen eine zweite Adresse (z.B. „Arbeit“) dafür wählt |
| `note` | nie überschrieben |

Die Zuordnung ist eine reine Funktion `contact_to_person(&ExtContact) -> PersonPatch` mit Unit-Tests. Der `synced_hash` in `external_links` beruht auf genau diesen Feldern, so funktioniert die Konfliktregel aus 2.2.

### 4.3 Google Kontakte (`google_contacts`)

- **API:** People API.
  - **Lesen:** `GET https://people.googleapis.com/v1/people/me/connections` mit diesen Parametern:
    - `personFields=names,emailAddresses,phoneNumbers,addresses,organizations,metadata`
    - `pageSize=1000`
    - `requestSyncToken=true`

    Danach liefert `syncToken` nur noch Änderungen; gelöschte Kontakte haben `metadata.deleted`. Mit **HTTP 410** ist der Token abgelaufen, dann folgt ein voller Abgleich.
  - **Verzögerung:** Änderungen erscheinen im Sync mit Minuten Verzögerung. Der Abgleich alle 30 min reicht dafür.
- **Anmeldung (`OAuthPkce`):**
  1. OAuth-Client vom Typ „Desktop“.
  2. Systembrowser öffnen (`tauri-plugin-opener`).
  3. Redirect auf `http://127.0.0.1:<zufälliger Port>`, mit PKCE (S256) und `state`.
  4. Kurzlebiger lokaler Empfänger (tokio, nur `127.0.0.1`, schließt nach dem ersten Aufruf).
  5. Code gegen Tokens tauschen.
  6. Refresh-Token in den Schlüsselbund (`google-contacts:refresh`), Access-Token nur im Speicher.

  Das Crate `oauth2` 5.x deckt PKCE und den Tausch ab. Desktop-Clients gelten bei Google als öffentlich, das Client-Secret ist kein Geheimnis.
- **Scope:** nur `https://www.googleapis.com/auth/contacts.readonly`.
- **Google-Prüfung, die Hürde:**
  - Kontakte sind ein **sensitiver Scope**.
  - Für eine veröffentlichte App mit eigenem Client muss Google den Client prüfen.
  - Bis dahin läuft er im Testmodus: höchstens 100 Testnutzer, und Refresh-Tokens laufen nach **7 Tagen** ab.

  Entschieden (18.11.2026): das Einfachste zuerst.
  - **Eigene Client-ID** als Option der Anbindung. Der Nutzer legt in seiner Google Cloud einen Desktop-Client an, die Anleitung kommt ins Handbuch. Sofort nutzbar und ohne Prüfung.
  - Damit die 7-Tage-Grenze nicht greift, wird das eigene Projekt auf „In Produktion“ gestellt. Für die eigene Nutzung braucht es dafür keine Prüfung; Google zeigt dann einen Hinweis „nicht überprüfte App“, den der Nutzer bestätigt. Das prüft der Spike K0.
  - Eine **mitgelieferte Client-ID** mit Google-Prüfung (Datenschutzseite, Begründung, Demovideo) ist zurückgestellt.
- **Drosselung:** 1 Anfrage pro Sekunde wie bei Bambu; Abgleich alle 30 min und beim Öffnen des Kunden-Dialogs.

### 4.4 Apple Kontakte – zwei Adapter, eine Fähigkeit

**a) `apple_contacts`: „Apple Kontakte (dieser Mac)“, nur macOS**
- Contacts-Framework über das Crate `objc2-contacts` (0.3), hinter `#[cfg(target_os = "macos")]`.
- **Zugriff:**
  - `CNContactStore.requestAccessForEntityType` (`AuthKind::OsPermission`)
  - `NSContactsUsageDescription` in der `Info.plist` (Tauri `bundle.macOS.infoPlist`)
  - Für signierte Builds mit Hardened Runtime das Entitlement `com.apple.security.personal-information.addressbook`
- **Lesen:** `CNContactFetchRequest` mit den nötigen Schlüsseln (Name, Firma, E-Mail, Telefon, Postadresse, Identifier). **Notizen nicht**, die brauchen ein eigenes, beschränktes Entitlement.

  Erfasst alle am Mac eingerichteten Konten, also auch iCloud, Exchange und Google. Änderungen per `CNChangeHistoryFetchRequest` oder, einfacher und für kleine Adressbücher ausreichend, voller Abgleich plus Hash.
- **Achtung:** Die macOS-Builds sind heute nicht signiert. Die Freigabe in „Datenschutz & Sicherheit → Kontakte“ hängt an der Code-Signatur und muss nach Updates eventuell neu erteilt werden. Darauf weist die App hin.

**b) `carddav`: „iCloud Kontakte (CardDAV)“, macOS und Windows** (entschieden: der Weg für Windows)
- **Server:** `https://contacts.icloud.com`. Anmeldung mit Apple-ID und **App-spezifischem Passwort** (`AuthKind::AppPassword`, erstellt unter account.apple.com). Das Passwort liegt nur im Schlüsselbund. Ändert der Nutzer sein Apple-ID-Passwort, wird es ungültig, und die App meldet „Anmeldung abgelaufen“.
- **Ablauf:**
  1. Erkennung per `current-user-principal` → `addressbook-home-set`.
  2. Adressbücher auflisten.
  3. `REPORT sync-collection` (RFC 6578) mit Sync-Token, sonst `addressbook-query`.
  4. vCard 3.0/4.0 lesen (kleiner eigener Parser für die benötigten Felder oder ein geprüftes Crate).
- Derselbe Adapter bedient jeden CardDAV-Server, etwa Nextcloud oder Synology. Das ist die erste Bestätigung, dass die Abstraktion trägt.
- **Risiko:** Es gibt Berichte über HTTP 401 mit frischen App-Passwörtern bei iCloud. Das klärt der Spike in K0.

### 4.5 Stufen für Kontakte

| Stufe | Inhalt | Fertig, wenn |
|---|---|---|
| K0 Spike (mit dir) | Drei kleine Testprogramme gegen deine Konten, Ergebnisse ohne Personendaten protokolliert. Google: OAuth-Loopback und erste Seite. Mac: Freigabe und Anzahl Kontakte. iCloud: CardDAV-Erkennung und Anzahl | jede Quelle liefert Kontakte; Feldformate (Adressen, Telefon) sind bekannt |
| K1 Import | `ContactSource` auf dem Gerüst aus A1, Zuordnung mit Tests. Oberfläche: „Anbindungen“ mit den drei Kontaktquellen; „Aus Kontakten …“ im Assistenten und auf der Kundenseite; Kennzeichen „verknüpft mit Google/Apple“ am Kunden | Kunde aus Kontakt anlegen und verknüpfen klappt mit allen drei Quellen |
| K2 Abgleich | Scheduler für Kontakte; Änderungen verknüpfter Kontakte übernehmen; Konfliktdialog; Verknüpfung lösen; gelöschter Kontakt → Hinweis, Kunde bleibt | Umzug im Adressbuch erscheint nach dem nächsten Abgleich beim Kunden |

Schreiben ins Adressbuch und die Google-Prüfung sind bewusst nicht geplant.

### 4.6 Tests

- **Rust, offline:**
  - Zuordnung `contact_to_person` (Straße/Hausnummer, Telefonwahl, Firmenkontakte)
  - Konfliktregel (Hash-Vergleich)
  - People API gegen `wiremock`: Seiten, `syncToken`, 410, `etag`-Konflikt bei `updateContact`
  - CardDAV gegen `wiremock` mit XML- und vCard-Fixtures (erfundene Daten)
  - OAuth: PKCE-Parameter und Loopback-Empfänger (nimmt nur `state` passend an)
- **Contract-Tests der Registry** mit einer Test-Anbindung: Schreibschutz, Drosselung, Status, Trennen löscht Geheimnisse
- **Mac-Adapter:** hinter `cfg`, manuelle Prüfliste (Freigabe erteilen/verweigern, leeres Adressbuch, iCloud-Konto)
- **`devmock.ts`:** Antworten für die `connector_*`-Befehle und Beispielkontakte (erfunden), damit die Oberfläche in der Browser-Vorschau prüfbar bleibt

### 4.7 Datenschutz

- Gelesen werden nur die genannten Felder. In der Datenbank liegen nur verknüpfte Kontakte, als Kunde plus Verknüpfung. Die Suchliste für „Aus Kontakten …“ bleibt im Speicher.
- Keine Personendaten in Logs, Fehlermeldungen oder Protokoll-Kopien. Bei Kontakten speichert das Protokoll nur Feldnamen und Hashes.
- „Trennen“ löscht Token oder Passwort aus dem Schlüsselbund. Auf Wunsch werden auch die Verknüpfungen gelöscht; die Kunden bleiben.

---

## 5. Aufwand (grob)

| Block | Aufwand |
|---|---|
| A1–A6 Bambu und Co. umziehen (inkl. Gerüst) | 6–8 Tage |
| K0 Spike Kontakte | ½–1 Tag, mit dir |
| K1 Import | 2–3 Tage |
| K2 Abgleich + Konflikte | 1½–2 Tage |

## 6. Entschieden (18.11.2026)

1. **Google:** das Einfachste zuerst, also eigene Client-ID des Nutzers. Keine Google-Prüfung vorerst.
2. **Apple unter Windows:** ja, iCloud über CardDAV mit App-spezifischem Passwort.
3. **Schreiben ins Adressbuch:** nein. Kontakte werden nur gelesen.
4. **Reihenfolge:** nicht Kontakte zuerst. Erst Bambu und Co. umziehen (A1–A6), dann die Kontakte (K0–K2).

Quellen:
- Google People API: [Contacts](https://developers.google.com/people/v1/contacts), [OAuth für Desktop-Apps](https://developers.google.com/identity/protocols/oauth2/native-app), [Prüfung sensitiver Scopes](https://developers.google.com/identity/protocols/oauth2/production-readiness/sensitive-scope-verification)
- Apple: [CNContactStore](https://developer.apple.com/documentation/contacts/cncontactstore), [Zugriff auf Kontakte](https://developer.apple.com/documentation/contacts/accessing-the-contact-store)
- iCloud CardDAV: [DAVx⁵: iCloud](https://davx5.com/tested-with/icloud)
