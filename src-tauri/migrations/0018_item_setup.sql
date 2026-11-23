-- Druckvorbereitung je Teil: Filament-Slots (Preset, Farbe, Spule) und Zuordnung der Objekte zu Slots.
ALTER TABLE item_estimates ADD COLUMN slots TEXT NOT NULL DEFAULT '[]';        -- JSON: [{slot, preset, material, color, spoolId}]
ALTER TABLE item_estimates ADD COLUMN objects TEXT NOT NULL DEFAULT '[]';      -- JSON: [{index, name, slot}]
ALTER TABLE item_estimates ADD COLUMN setup_state TEXT NOT NULL DEFAULT '';    -- '' | entwurf | bereit | uebergeben
ALTER TABLE item_estimates ADD COLUMN project_file TEXT NOT NULL DEFAULT '';   -- erzeugtes Projekt im Auftragsordner (D2)
