-- Druckprofil und Schätzung je Teil, bevor gesliced im Auftragsordner liegt (Kalkulation vorab).
CREATE TABLE item_estimates (
    item_id      TEXT PRIMARY KEY,
    source       TEXT NOT NULL DEFAULT '',    -- Profilquelle, z.B. printerprofiles
    printer      TEXT NOT NULL DEFAULT '',    -- Drucker-Kennung, z.B. h2c
    nozzle       TEXT NOT NULL DEFAULT '',
    process      TEXT NOT NULL DEFAULT '',    -- Datei des Prozess-Presets in der Quelle
    filament     TEXT NOT NULL DEFAULT '',    -- Datei des Filament-Presets in der Quelle
    bed_type     TEXT NOT NULL DEFAULT '',
    basis        TEXT NOT NULL DEFAULT '',    -- slicer | makerworld (leer = keine Schätzung)
    seconds      BIGINT,
    filaments    TEXT NOT NULL DEFAULT '[]',  -- JSON wie bei Drucken: [{type, color, grams}]
    mw_instance  TEXT NOT NULL DEFAULT '',    -- gewähltes MakerWorld-Druckprofil
    error        TEXT NOT NULL DEFAULT '',
    updated_at   TEXT
);
