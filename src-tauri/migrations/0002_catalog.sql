-- Katalog mit fertigen Projekten (3MF/STL), Herkunft aus Links

CREATE TABLE catalog (
    id            TEXT PRIMARY KEY,
    title         TEXT NOT NULL,
    description   TEXT NOT NULL DEFAULT '',
    file_name     TEXT NOT NULL,           -- im Ordner catalog/<id>/
    thumb_name    TEXT,                    -- Vorschaubild im selben Ordner
    source_url    TEXT,
    designer      TEXT NOT NULL DEFAULT '',
    license       TEXT NOT NULL DEFAULT '',
    plates        BIGINT NOT NULL DEFAULT 1,
    print_seconds BIGINT,                  -- nur bei gesliced gespeicherten 3MF
    weight_grams  BIGINT,
    filaments     TEXT NOT NULL DEFAULT '[]',  -- JSON: [{"type":"PLA","color":"#RRGGBB"}]
    tags          TEXT NOT NULL DEFAULT '',
    hidden        BIGINT NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- Teil eines Auftrags kann auf seine Druckdatei (Anhang) verweisen
ALTER TABLE order_items ADD COLUMN file_attachment_id TEXT;
