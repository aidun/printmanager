-- Rückmeldung des Druckers (über das Bambu-Konto) zu einem Druck. Die App schlägt das Ergebnis nur vor.
ALTER TABLE print_runs ADD COLUMN printer_name TEXT NOT NULL DEFAULT '';
ALTER TABLE print_runs ADD COLUMN printer_status TEXT NOT NULL DEFAULT '';   -- '' | ok | fehldruck
ALTER TABLE print_runs ADD COLUMN printer_finished_at TEXT;
ALTER TABLE print_runs ADD COLUMN actual_seconds BIGINT;                      -- echte Druckdauer
ALTER TABLE print_runs ADD COLUMN actual_filaments TEXT NOT NULL DEFAULT '[]'; -- echte Gramm pro Slot (RunFilament)
ALTER TABLE print_runs ADD COLUMN printer_task_id TEXT;                     -- Eintrag im Druckverlauf (nur einmal zuordnen)
