-- Schreibprotokoll für alle Anbindungen (docs/PLAN-Anbindungen.md, Stufe A6); übernimmt filament_writes.
CREATE TABLE connector_writes (
    id          TEXT PRIMARY KEY,
    created_at  TEXT NOT NULL,
    connector   TEXT NOT NULL,                -- z.B. bambu-cloud
    entity      TEXT NOT NULL,                -- z.B. spool
    local_id    TEXT NOT NULL,
    label       TEXT NOT NULL DEFAULT '',
    action      TEXT NOT NULL,                -- rest | anlegen | loeschen | slot
    old_value   TEXT,
    new_value   TEXT,
    status      TEXT NOT NULL,                -- ok | konflikt | fehler | wartet | uebersprungen
    message     TEXT NOT NULL DEFAULT '',
    snapshot    TEXT                          -- Zustand beim Anbieter vor der Änderung (JSON)
);
CREATE INDEX connector_writes_time ON connector_writes (created_at);

INSERT INTO connector_writes (id, created_at, connector, entity, local_id, label, action, old_value, new_value, status, message, snapshot)
SELECT id, created_at, 'bambu-cloud', 'spool', spool_id, label, action, old_value, new_value, status, message, snapshot FROM filament_writes;

DROP TABLE filament_writes;
