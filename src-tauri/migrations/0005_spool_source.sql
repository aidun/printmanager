-- Filament aus dem Bambu-Konto: Herkunft der Spule und wo sie gerade steckt (AMS).
ALTER TABLE spools ADD COLUMN source TEXT NOT NULL DEFAULT 'datei';  -- datei | konto
ALTER TABLE spools ADD COLUMN source_updated_at TEXT;               -- Änderungszeit laut Quelle
ALTER TABLE spools ADD COLUMN in_printer BIGINT NOT NULL DEFAULT 0;
ALTER TABLE spools ADD COLUMN printer_name TEXT NOT NULL DEFAULT '';
ALTER TABLE spools ADD COLUMN ams_slot TEXT NOT NULL DEFAULT '';    -- z.B. „AMS 1 · Slot 2“
ALTER TABLE spools ADD COLUMN depleted BIGINT NOT NULL DEFAULT 0;
