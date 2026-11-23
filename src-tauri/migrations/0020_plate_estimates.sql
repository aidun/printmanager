-- Schätzung je Platte, damit die Plattenwahl ohne neues Slicen neu gerechnet werden kann.
ALTER TABLE item_estimates ADD COLUMN plate_results TEXT NOT NULL DEFAULT '[]';  -- JSON: [{index, seconds, filaments}]
