-- MakerWorld-Sammlungen im Katalog. Einträge aus MakerWorld dürfen ohne Datei sein (file_name = '');
-- die 3MF lädt die App erst bei Bedarf.
ALTER TABLE catalog ADD COLUMN origin TEXT NOT NULL DEFAULT 'lokal';      -- lokal | makerworld
ALTER TABLE catalog ADD COLUMN remote_id TEXT;                           -- MakerWorld-Design
ALTER TABLE catalog ADD COLUMN remote_instance_id TEXT;                  -- Druckprofil der geladenen 3MF
ALTER TABLE catalog ADD COLUMN cover_url TEXT;
ALTER TABLE catalog ADD COLUMN remote_missing BIGINT NOT NULL DEFAULT 0;  -- in keiner Sammlung mehr
ALTER TABLE catalog ADD COLUMN remote_synced_at TEXT;
CREATE INDEX catalog_remote ON catalog (remote_id);

CREATE TABLE mw_collections (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    design_cnt  BIGINT NOT NULL DEFAULT 0,
    is_default  BIGINT NOT NULL DEFAULT 0,
    enabled     BIGINT NOT NULL DEFAULT 1,
    synced_at   TEXT
);

CREATE TABLE catalog_collections (
    catalog_id    TEXT NOT NULL REFERENCES catalog(id) ON DELETE CASCADE,
    collection_id TEXT NOT NULL REFERENCES mw_collections(id) ON DELETE CASCADE,
    PRIMARY KEY (catalog_id, collection_id)
);
