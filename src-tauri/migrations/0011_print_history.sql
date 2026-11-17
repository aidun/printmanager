-- Druckverlauf aus dem Bambu-Konto (lokale Kopie von my/tasks). Gramm sind Slicer-Werte, die Dauer ist echt.
CREATE TABLE print_history (
    id              TEXT PRIMARY KEY,            -- ID des Eintrags im Konto
    title           TEXT NOT NULL DEFAULT '',    -- Druckauftrag, z.B. „A0004 benchy_plate_1“
    design_title    TEXT NOT NULL DEFAULT '',    -- MakerWorld-Modell, falls vorhanden
    design_id       TEXT,
    device_name     TEXT NOT NULL DEFAULT '',
    device_model    TEXT NOT NULL DEFAULT '',
    plate_index     BIGINT,
    status          BIGINT NOT NULL DEFAULT 0,   -- 2 fertig, 3 abgebrochen/Fehler, sonst laufend/unbekannt
    start_time      TEXT,
    end_time        TEXT,
    planned_seconds BIGINT,                      -- Schätzung des Slicers
    weight_grams    DOUBLE PRECISION,
    filaments       TEXT NOT NULL DEFAULT '[]',  -- JSON RunFilament je AMS-Slot
    cover_url       TEXT,
    order_id        TEXT REFERENCES orders(id) ON DELETE SET NULL,
    matched_by      TEXT,                        -- manuell | nummer | datei | titel
    synced_at       TEXT NOT NULL
);
CREATE INDEX print_history_end ON print_history (end_time);
CREATE INDEX print_history_order ON print_history (order_id);
