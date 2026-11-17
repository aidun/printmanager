-- Geschätzter tatsächlicher Verbrauch bei abgebrochenen Drucken (Konto meldet nur den geplanten Wert).
ALTER TABLE print_history ADD COLUMN progress_at_end BIGINT;          -- zuletzt gesehener Fortschritt (live erfasst)
ALTER TABLE print_history ADD COLUMN used_factor DOUBLE PRECISION;    -- Anteil des geplanten Verbrauchs (1.0 = vollständig)
ALTER TABLE print_history ADD COLUMN used_method TEXT;                -- fertig | fortschritt | zeit | geplant

-- Fortschritt laufender Drucke, wie ihn die App gesehen hat (für spätere Abbrüche)
CREATE TABLE print_progress (
    task_key  TEXT NOT NULL,
    device    TEXT NOT NULL,
    progress  BIGINT NOT NULL,
    seen_at   TEXT NOT NULL,
    PRIMARY KEY (task_key, device)
);
