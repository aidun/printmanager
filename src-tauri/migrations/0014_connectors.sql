-- Einheitliche Schicht für externe Anbindungen (docs/PLAN-Anbindungen.md, Stufe A1)

-- Verknüpfung eines lokalen Datensatzes mit seinem Gegenstück bei einer Anbindung
CREATE TABLE external_links (
    id                TEXT PRIMARY KEY,
    connector         TEXT NOT NULL,
    entity            TEXT NOT NULL,
    external_id       TEXT NOT NULL,
    local_id          TEXT NOT NULL,
    etag              TEXT NOT NULL DEFAULT '',
    remote_updated_at TEXT,
    synced_hash       TEXT NOT NULL DEFAULT '',
    local_hash        TEXT NOT NULL DEFAULT '',
    last_synced_at    TEXT,
    UNIQUE (connector, entity, external_id)
);
CREATE INDEX external_links_local ON external_links (entity, local_id);

-- Kleine Zustandswerte je Anbindung, z.B. Sync-Token oder Cursor
CREATE TABLE connector_state (
    connector  TEXT NOT NULL,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (connector, key)
);
