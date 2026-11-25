# Plan: Kern als Bibliothek, Standalone oder Server, Begleit-App für iOS

Stand 24.11.2026. Noch nichts umgesetzt. Löst `PLAN-iOS.md` ab (umbenannt und erweitert).

## Ziel

Printmanager soll in zwei Betriebsarten laufen, mit derselben Fachlogik und derselben Oberfläche:

- **Standalone:** wie heute auf einem MacBook, mit SQLite und lokalen Dateien, ohne Server und ohne Anmeldung.
- **Server:** ein zentraler Server auf dem NAS, den mehrere Geräte und mehrere Benutzer eines Teams gemeinsam nutzen:
  - Macs am Arbeitsplatz, auf denen geslict wird
  - iPhone und iPad, auch offline
  - der Browser

Dafür wird der Kern zur Bibliothek. Die Desktop-App ruft ihn direkt auf, alle anderen Geräte über eine API.

## Grundsätze

1. **Jeder Befehl wird nur einmal definiert, im Kern.** Tauri und der Server sind dünne Transporte darüber. So können die beiden Wege nicht auseinanderlaufen.
2. **Die Datenbank ist die Quelle für alles, was angezeigt oder für eine Entscheidung gebraucht wird.** Originaldateien (3MF, STL, G-Code, Fotos) liegen im Speicher: Standalone ist das ein lokaler Ordner, im Serverbetrieb S3. Jeder Mac hält davon eine lokale Kopie für den Slicer.
3. **Im Serverbetrieb spricht nur der Server mit Postgres, S3 und dem Bambu-Konto.** Die Geräte kennen nur die API.
4. **Die harten Regeln aus `AGENTS.md` gelten in beiden Betriebsarten:** keine Befehle an Drucker, das Konto nur lesen (Ausnahme `filament_write`), nie in Dateien von Bambu Studio schreiben.
5. **Ein Team, gemeinsame Daten:**
   - Anmeldung und Protokoll, wer etwas geändert hat.
   - Keine Rollen, keine Trennung der Daten.
   - Ein Bambu-Konto für die ganze Werkstatt.

## Aufbau

```
Cargo-Workspace im Repo-Wurzelordner
crates/core      Fachlogik ohne Tauri: Store, Modelle, 3MF, Matching, Verbrauch, Filament,
                 Link-Resolver, Anbindungen (HTTP, Bambu, MakerWorld, Printables), Preise,
                 Speicher (lokal | S3), Dateiaufträge, Verteiler `call`
crates/server    axum: Anmeldung, POST /api/{befehl}, SSE-Ereignisse, Dateien, liefert die Weboberfläche
src-tauri        Desktop-Hülle: Fenster, Ordner beobachten, Bambu Studio (öffnen, slicen),
                 Kontakte, Schlüsselbund, Anmeldefenster fürs Bambu-Konto
src/             React; `api.ts` wählt den Transport: direkt (Tauri) oder HTTP (Server)
```

| | Standalone (MacBook) | Server |
|---|---|---|
| Datenbank | SQLite im Datenordner | Postgres auf dem NAS, nur der Server greift zu |
| Dateien | lokaler Ordner (`orders/`, `catalog/`) | S3 auf dem NAS, Macs halten lokale Kopien |
| Bambu-Konto, MakerWorld, Link-Downloads | App (Kern im Prozess) | Server |
| Druck erkennen (3MF auswerten) | App, wie heute | Server, nach dem Upload |
| Ordner beobachten, Bambu Studio, Kalkulation per Slicer | App | App am Arbeitsplatz-Mac |
| Kontakte (Apple) | App | App des Geräts, Ergebnis per API |
| Anmeldung | keine (lokaler Benutzer) | Benutzer und ein Token je Gerät |
| iPhone/iPad, Browser | – | API-Client |

## Schritte

### 0. Kern herauslösen (zuerst, ohne sichtbare Änderung)

Heute hängen vor allem diese Teile an Tauri (`AppHandle`, `State`): `commands.rs` (93 Stellen), `estimates.rs`, `bambu/*`, `connectors/commands.rs`, `contactsync.rs`. Meist geht es dabei um Zustand und Ereignisse.

1. **Cargo-Workspace** mit `crates/core` und `src-tauri`.
   - Diese Module ziehen nach und nach in den Kern: `store.rs`, `model.rs`, `threemf.rs`, `matching.rs`, `usage.rs`, `filament.rs`, `linkimport.rs`, `connectors/`, `bambu/`, `profiles.rs`, `pricing.rs`.
   - Die Migrationen ziehen mit.
2. **Kontext statt `AppHandle`:** `Core { db, data_dir, registry, storage, events: Arc<dyn EventSink> }` und je Aufruf ein `Caller { user, device }`.
   - Die Ereignisse `pm:changed` und `pm:external` gehen an den `EventSink`. In Tauri wird daraus `emit`, auf dem Server SSE.
   - Was nur auf dem Desktop geht (Slicer, Ordner, Kontakte), bleibt hinter Fähigkeiten nach dem Muster aus [PLAN-Anbindungen.md](PLAN-Anbindungen.md). Der Kern fragt etwa „kann slicen?“, statt Bambu Studio direkt zu rufen.
3. **Verteiler:** `core.call(befehl, args: JSON, caller) -> JSON`.
   - Die Befehlsliste trägt die Kennzeichen „nur lesen“ und „nur Desktop“.
   - `READ_ONLY` in `api.ts` wird aus dieser Liste erzeugt oder per Test mit ihr abgeglichen.
4. **Tauri:** Ein Befehl `call` reicht an den Kern weiter. Eigene Tauri-Befehle bleiben nur für Desktop-Dinge: Fenster, Ordner zeigen, in Bambu Studio öffnen, Anmeldung am Bambu-Konto.
   - Der Umzug geht Befehl für Befehl. Jeder Zwischenstand bleibt lauffähig.
5. **`api.ts`:** `invoke` bekommt einen Transport. Standalone ist das `tauriInvoke("call", …)`, im Serverbetrieb `fetch("/api/{befehl}")`. `devmock.ts` bleibt, wie er ist.
6. **Prüfung:**
   - `cargo test` (die Store-Tests ziehen mit in den Kern) und `npm run test:e2e` laufen unverändert grün.
   - Die Dev-App wird von Hand geprüft.
   - Ein neuer Test prüft, dass jeder Befehl aus `api.ts` im Verteiler existiert.

### A. Vorschauen und 3MF-Daten in der Datenbank

1. **Vorschaubilder:** neue Tabelle `blobs` (id, sha256, mime, bytes, created_at), WebP oder JPEG bei etwa 400 px, rund 20–50 KB.
   - Die Tabelle ist eigen, damit Listen die Bytes nicht mitladen.
   - Verweise kommen aus den Platten (heute `plateSrc`), aus dem Katalog (heute `thumbName`) und aus Bild-Anhängen (heute `attachments.path`).
   - Die Oberfläche lädt Bilder über den Befehl `blob_get` bzw. `GET /api/blobs/{id}` statt über `convertFileSrc`.
2. **Was aus einer 3MF gelesen wird, wird gespeichert:** Platten, Objekte, Filamente, Gramm, Druckzeit, ob gesliced. Dazu Prüfsumme und Änderungszeit.
   - Neu gelesen wird nur, wenn sich die Datei geändert hat.
   - Auch das Ergebnis von `threemf::analyze` beim Hinzufügen einer Datei wird gespeichert.
3. **Einmaliger Lauf, der Vorschauen nachträgt:** für bestehende Aufträge und den Katalog.

### B. Dateiaufträge

Eine Warteschlange `file_jobs` mit den Spalten:
- `id`
- `order_id`
- `kind`: `download_link` | `download_makerworld`
- `payload` (JSON: Link, gewählte Dateien)
- `state`: `pending` | `done` | `failed`
- `error`
- `claimed_at`, `claimed_by`
- `created_at`, `done_at`

1. **`link_import_to_order` teilen:**
   - Vorschau und Auftrag sofort anlegen, die Dateien als Job eintragen.
   - Den Job arbeitet ein Worker im Kern ab: Standalone in der App, im Serverbetrieb auf dem Server.
2. **Worker:** Er beansprucht einen Job per `UPDATE … SET claimed_at = … WHERE id = $1 AND claimed_at IS NULL`, lädt mit `download_file` bzw. `makerworld::fetch_file` und legt die Datei im Speicher ab.
3. **Zustand im Auftrag:** „Dateien werden geladen“, „Dateien da“, bei einem Fehler Klartext mit „Erneut versuchen“.

Das nützt sofort, auch standalone: Der Auftrag ist sofort angelegt, selbst wenn der Download langsam ist.

### G. Speicher: lokal oder S3

Ziel: Alle Dateien sind auf allen Geräten aktuell, und Bambu Studio arbeitet trotzdem mit lokalen Dateien.

**Speicher-Schnittstelle im Kern**
- Zwei Umsetzungen: `LocalStorage` für den heutigen Ordner (standalone) und `S3Storage` (Server).
- Crate: `object_store`, das beides abdeckt und bedingt schreiben kann.
- S3-kompatibler Speicher auf dem NAS, zum Beispiel MinIO oder Garage im Docker.
  - Er ist nur über Tailscale erreichbar, ohne TLS. Die Anfragen sind trotzdem signiert (Signature V4).
  - Auf dem Bucket ist die Versionierung eingeschaltet.
  - Die Zugangsdaten kennt nur der Server.
- Geräte laden Dateien über die API, und zwar über zeitlich begrenzte, signierte URLs (presigned) direkt aus S3 bzw. nach S3. Große 3MF laufen so nicht durch den Server.
- Vorschaubilder bleiben als `blobs` in der Datenbank (A).

**Tabelle `files`**
- id
- `order_id` bzw. `catalog_id`
- `rel_path`: relativ zum Auftrags- bzw. Katalogordner, wie heute `files::order_dir`
- `size`, `sha256`
- `s3_key`, `etag`
- `updated_at`, `updated_by` (Benutzer und Gerät)
- `deleted_at`

**Lokale Kopie auf Macs am Arbeitsplatz**
- Das ist der heutige Ordner `orders/NNNN/` bzw. `catalog/<id>/`. `watch.rs` und Bambu Studio bleiben unverändert.
- Offene Aufträge werden vollständig vorgehalten, andere bei Bedarf geladen. Archivierte darf die App lokal entfernen.

**Abläufe**
1. **Herunterladen:** Fehlt eine Datei oder weicht ihre `sha256` ab, wird sie geladen. Das Laden geht in eine Temp-Datei, die danach umbenannt wird. So sieht `watch.rs` nie eine halbe Datei.
2. **Hochladen:**
   - `watch.rs` meldet eine Änderung. Die App wartet, bis die Größe stabil ist, berechnet die `sha256` und lädt die Datei mit `If-Match` auf den bekannten `etag` hoch, bzw. mit `If-None-Match: *` bei neuen Dateien.
   - Dateien, die der Download selbst geschrieben hat, lädt die App nicht wieder hoch. Das erkennt sie an der `sha256`.
3. **Konflikt:** Schlägt der Upload fehl, weil inzwischen ein anderes Gerät gespeichert hat, bleibt die eigene Version als „figur (MacBook).3mf“ erhalten. Die App meldet das im Klartext: „Zwei Versionen von figur.3mf, bitte eine behalten.“
4. **Löschen:** `deleted_at` setzen, das Objekt in S3 entfernen (die Versionierung hält es vor) und die Datei auf den anderen Geräten in `trash/` verschieben.
5. **Druck erkennen:**
   - Im Serverbetrieb wertet der Server die 3MF nach dem Upload aus, mit `threemf` im Kern. Ein Druck wird so nur einmal erkannt, egal auf welchem Mac geslict wurde.
   - Standalone läuft es wie heute.
6. **Oberfläche:** Jede Datei im Auftrag zeigt ihren Stand: lokal, wird geladen, wartet auf Upload oder Konflikt. Ohne Verbindung zeigt die App einen Hinweis, und die lokalen Dateien bleiben nutzbar.

### S. Server

1. **`crates/server` mit axum:**
   - `POST /api/{befehl}` ruft `core.call`.
   - `GET /api/events` liefert Ereignisse per SSE. Damit ersetzen sie `pm:changed` und `pm:external`.
   - Dazu Endpunkte für Blobs und für signierte Datei-URLs.
   - Der Server liefert auch die Weboberfläche aus (`npm run build` mit HTTP-Transport).
2. **Anmeldung:**
   - Tabelle `users` (id, name, Passwort-Hash mit argon2, created_at) und `devices` (id, user_id, name, token_hash, last_seen).
   - Das Token liegt auf dem Gerät im Schlüsselbund.
   - Den ersten Benutzer legt `printmanager-server init` an.
3. **Mehrere Benutzer gleichzeitig:**
   - Spalten `created_by` und `updated_by` an Aufträgen, Kunden, Spulen und Dateien. Die Oberfläche zeigt dann etwa „von Anna geändert“.
   - Nummern für Aufträge und Kunden mit `UNIQUE`-Index und Wiederholung bei Konflikt. Heute werden sie als `MAX(number)+1` vergeben.
   - Speichern mit `updated_at` aus dem Bearbeitungsstand. Hat sich der Datensatz inzwischen geändert, meldet die App „Inzwischen von Anna geändert“ und zeigt den neuen Stand.
   - Gleichzeitige Aufrufe desselben Vorgangs: Jeder schreibende Aufruf trägt eine `request_id`. Die Tabelle `processed_requests` verhindert, dass er doppelt ausgeführt wird.
4. **Versionen:** Der Client schickt seine API-Version mit. Passt sie nicht, meldet die App „App aktualisieren“. Migrationen spielt nur der Server ein.
5. **Bambu-Konto auf dem Server:**
   - Ein Konto für das Team, gedrosselt auf höchstens 1 Anfrage pro Sekunde im Serverprozess.
   - Die Anmeldung läuft am Desktop über das bekannte Fenster. Die App übergibt das Token einmalig an den Server.
   - Der Server speichert es in einer Datei im eigenen Datenordner mit Rechten 0600, nie in der Datenbank, in Logs oder in Antworten.
   - `filament_write` bleibt standardmäßig aus und braucht wie bisher eine Bestätigung im Dialog.
6. **Betrieb auf dem NAS:**
   - `docker-compose` mit Server, Postgres und MinIO.
   - Alles nur auf der Tailscale-Adresse, ohne TLS.
   - Postgres und MinIO sind nur im Docker-Netz erreichbar, nicht im Tailnet.
   - Sicherung: `pg_dump` im Takt plus Versionierung des Buckets.
7. **Desktop im Serverbetrieb:**
   - Einstellungen → Betrieb: „Auf diesem Mac“ oder „Server“, mit Adresse und Anmeldung.
   - Im Serverbetrieb nutzt die App den HTTP-Transport. Sie behält aber ihre Desktop-Fähigkeiten: Ordner beobachten, Bambu Studio, Kalkulation per Slicer und Kontakte. Deren Ergebnisse schickt sie per API an den Server.

### U. Umzug von Standalone auf den Server

- Ein Befehl in der Desktop-App lädt die eigene SQLite-Datenbank und die Dateien zum Server hoch: Tabelle für Tabelle, Dateien nach S3.
- Danach prüft er die Zeilenzahl und die `sha256` jeder Datei.
- Vorher sichert er die SQLite-Datenbank. Danach schaltet die App auf „Server“ um.
- Der umgekehrte Weg (Export vom Server in eine Standalone-Datenbank) dient als Sicherung und Ausweg.

### F. Offline-Betrieb auf iOS

Ansatz: **Lokaler Zwischenspeicher plus Postausgang**, beides mit dem Kern.

- **Lokale SQLite auf dem Gerät** mit denselben Migrationen.
- **Lesen:**
  - Bei Verbindung holt die App einen Stand vom Server (`snapshot`, später nur Änderungen seit einem Zeitpunkt). Vorschaubilder kommen für offene Aufträge mit.
  - Befehle, die nur lesen, laufen immer lokal über `core.call` gegen den Zwischenspeicher, mit dem Hinweis „Stand: vor 2 Std.“.
- **Schreiben:**
  - Ein schreibender Befehl läuft sofort lokal über `core.call`.
  - Zusätzlich kommt er mit `request_id` in die Tabelle `outbox` (id, befehl, args JSON, base_updated_at, state, error, created_at).
  - Dafür braucht es keine eigenen Vorgangstypen. Der Postausgang speichert Befehle.
- **Nachsenden:** Bei Verbindung schickt die App die Befehle der Reihe nach an `POST /api/{befehl}`. Dank `request_id` und den UUIDs, die schon auf dem Gerät entstehen, schadet es nicht, wenn ein Befehl zweimal ankommt.
- **Nummern:** Neue Aufträge und Kunden zeigen „Neu“, bis der Server die Nummer vergibt.
- **Konflikte:** Hat sich ein Datensatz auf dem Server inzwischen geändert, gewinnt der Server. Die App meldet das im Klartext: „A0012 wurde von Anna geändert, dein Status wurde nicht übernommen.“
- **Fotos:** Offline gehen sie als Datei in den Postausgang und werden später über eine signierte URL hochgeladen.
- **Oberfläche:** Ein Symbol zeigt den Stand der Synchronisation („3 Änderungen warten“). Eine Liste zeigt die fehlgeschlagenen Befehle mit „Erneut“ und „Verwerfen“.

### E. iOS-App

1. `tauri ios init` mit eigener Kennung `de.printmanager.app.ios` und eigener Konfiguration nach dem Muster von `tauri.dev.conf.json`.
2. Die App nutzt den Kern nur für den Zwischenspeicher (F). Nach außen spricht sie ausschließlich mit der API. Sie hat keinen Zugang zu Postgres oder S3, kein Bambu-Token und keinen Slicer.
3. Was nur auf dem Desktop geht, wird abgegrenzt (`watch.rs`, `notify`, `slicers/`, `connectors/slicer.rs`, `Command::new("open")`). Das ergibt sich weitgehend schon aus Schritt 0, weil diese Teile in `src-tauri` bleiben.
4. Links und Dateien öffnet bzw. teilt sie über Tauri-Plugins.
5. Kontakte: `objc2-contacts` auch für iOS, mit `NSContactsUsageDescription` in der Info.plist.
6. **Oberfläche:**
   - Die Ansichten blenden Aktionen nach Fähigkeiten aus (`canSlice`, `canWatch` …).
   - Layout fürs iPhone: Tab-Leiste statt Seitenleiste, Board als Liste nach Status, kein Drag & Drop.
   - Das iPad liegt nahe am heutigen Layout.
7. **Verteilung:** Apple-Entwicklerkonto (99 €/Jahr) und TestFlight.

| Bereich | iOS |
|---|---|
| Heute, Board, Liste, Auftrag ansehen | ja, auch offline |
| Auftrag anlegen, auch per Link | ja, auch offline. Dateien lädt der Server (B). |
| Status, Termin, Kunde ändern; Fotos anhängen | ja, auch offline |
| Dateien ansehen und teilen | ja, bei Bedarf geladen |
| Filament, Drucker, Druckverlauf ansehen | ja, nur lesen |
| Kunden und Kontakte | ja |
| Slicen, Kalkulation, Druckvorbereitung | nein, nur Ergebnisse ansehen |

## Reihenfolge

1. **0:** Kern herauslösen. Danach ist die App standalone unverändert nutzbar.
2. **A:** Vorschauen und 3MF-Daten in der Datenbank. Nützt sofort.
3. **B:** Dateiaufträge. Nützt sofort.
4. **G:** Speicher-Schnittstelle, standalone zunächst nur `LocalStorage`.
5. **S:** Server mit Anmeldung, SSE, `S3Storage`, Docker auf dem NAS. Getestet mit der Dev-App im Serverbetrieb und dem Browser.
6. **U:** Umzug der echten Daten. Nur mit Zustimmung des Nutzers und nach einer Sicherung.
7. **F:** Offline-Zwischenspeicher und Postausgang, zuerst als Tests im Kern.
8. **E:** iOS-App, zuerst iPad, dann iPhone.

## Prüfung (je Schritt)

- `npx tsc -b`, `npm run test:e2e`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`. Die CI muss dafür auf den Workspace umgestellt werden.
- Store-Tests mit einer Temp-Datenbank für `blobs`, `files`, `file_jobs`, `users`, `processed_requests` und `outbox`.
- Speicher-Tests gegen das Speicher-Backend von `object_store` (offline), optional gegen MinIO (`PM_S3_TESTS=1`).
- Server-Tests: Anmeldung, `call` über HTTP, doppelte `request_id`, Nummernvergabe bei gleichzeitigen Aufrufen, Konflikt bei `updated_at`.
- E2E zusätzlich mit HTTP-Transport gegen einen Testserver mit Beispieldaten.
- Offline-Test im Simulator: Postausgang ohne Verbindung füllen, verbinden, Ergebnis prüfen.

## Entscheidungen

- 23.11.2026: Postgres auf dem NAS bzw. Heimserver, nur über Tailscale erreichbar, ohne TLS.
- 23.11.2026: Vorschaubilder in der Datenbank (`blobs`).
- 23.11.2026: iOS ist von Anfang an offline nutzbar.
- 24.11.2026: Originaldateien in S3 statt Syncthing. So braucht iOS keine Zusatz-App, und die App zeigt den Stand jeder Datei.
- 24.11.2026: Kern als Bibliothek, Betrieb standalone oder mit Server und API. Mehrere Benutzer als Team mit gemeinsamen Daten, ohne Rollen und ohne Mandanten.
- 24.11.2026: Geräte sprechen im Serverbetrieb nur mit der API, nie direkt mit Postgres oder S3.

## Offen

- **Harte Regel 4 in `AGENTS.md`** für den Serverbetrieb neu fassen. Vorschlag: „Das Bambu-Token liegt standalone nur im Schlüsselbund, im Serverbetrieb nur in einer Datei mit Rechten 0600 im Datenordner des Servers. Nie in Datenbank, Logs, Antworten, Tests oder Commits.“ Wird erst mit Schritt S geändert.
- Sollen Aufrufe über die API protokolliert werden? Die Tabelle `events` gibt es schon. Offen ist, ob jeder schreibende Aufruf mit Benutzer dort landet.
