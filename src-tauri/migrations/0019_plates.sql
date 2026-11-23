-- Platten einzeln verfolgen: je Druck die Platten mit geplanten und tatsächlichen Werten;
-- je Teil die Platten, die für die Kalkulation vorab zählen.
ALTER TABLE print_runs ADD COLUMN plate_details TEXT NOT NULL DEFAULT '[]';  -- JSON: [{index, seconds, filaments, status, actualSeconds, actualFilaments, taskId, printerName, finishedAt}]
ALTER TABLE item_estimates ADD COLUMN plates TEXT NOT NULL DEFAULT '[]';     -- JSON: gewählte Platten, leer = alle
