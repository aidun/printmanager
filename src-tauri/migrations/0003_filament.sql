-- Filament-Bibliothek: Spiegel von Bambu Studios spools.json plus eigene Felder.
-- Die App schreibt nie in spools.json; Restmengen führt sie hier selbst.

CREATE TABLE spools (
    id              TEXT PRIMARY KEY,          -- spool_id aus Bambu Studio
    brand           TEXT NOT NULL DEFAULT '',
    series          TEXT NOT NULL DEFAULT '',
    material        TEXT NOT NULL DEFAULT '',
    color           TEXT NOT NULL DEFAULT '',  -- #RRGGBB
    color_name      TEXT NOT NULL DEFAULT '',
    initial_grams   DOUBLE PRECISION NOT NULL DEFAULT 0,
    studio_grams    DOUBLE PRECISION NOT NULL DEFAULT 0,  -- zuletzt in Bambu Studio gesehener Rest
    remaining_grams DOUBLE PRECISION NOT NULL DEFAULT 0,  -- Rest laut App
    studio_status   TEXT NOT NULL DEFAULT '',
    hidden          BIGINT NOT NULL DEFAULT 0,
    missing         BIGINT NOT NULL DEFAULT 0, -- nicht mehr in spools.json
    note            TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- Ein Druck(versuch): Ergebnis einer gesliced gespeicherten 3MF im Auftragsordner
CREATE TABLE print_runs (
    id            TEXT PRIMARY KEY,
    order_id      TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    item_id       TEXT,
    attachment_id TEXT,
    file_name     TEXT NOT NULL,
    plates        BIGINT NOT NULL DEFAULT 1,
    print_seconds BIGINT,
    weight_grams  DOUBLE PRECISION,
    filaments     TEXT NOT NULL DEFAULT '[]',  -- JSON: [{"type","color","grams","spoolId"}]
    signature     TEXT NOT NULL DEFAULT '',    -- erkennt erneutes Speichern ohne neues Slicen
    result        TEXT NOT NULL DEFAULT 'offen', -- offen | ok | fehldruck
    deducted      BIGINT NOT NULL DEFAULT 0,
    note          TEXT NOT NULL DEFAULT '',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX print_runs_order ON print_runs(order_id);
