-- Schreiben ins Bambu-Konto (abschaltbar): RFID-Kennung der Spule und Protokoll aller Schreibvorgänge.
ALTER TABLE spools ADD COLUMN rfid TEXT NOT NULL DEFAULT '';

CREATE TABLE filament_writes (
    id          TEXT PRIMARY KEY,
    created_at  TEXT NOT NULL,
    spool_id    TEXT NOT NULL,
    label       TEXT NOT NULL DEFAULT '',     -- Spule zum Anzeigen (Marke, Name, Farbe)
    action      TEXT NOT NULL,                -- rest | anlegen | loeschen | rueckgaengig
    old_value   TEXT,
    new_value   TEXT,
    status      TEXT NOT NULL,                -- ok | konflikt | fehler | wartet | uebersprungen
    message     TEXT NOT NULL DEFAULT '',
    snapshot    TEXT                          -- Spule im Konto vor der Änderung (JSON), für Rückgängig/Löschen
);
CREATE INDEX filament_writes_time ON filament_writes (created_at);
