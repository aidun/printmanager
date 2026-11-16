-- Archiv: abgeschlossene Aufträge verschwinden aus Board und Liste, bleiben aber in Suche und Statistik.
ALTER TABLE orders ADD COLUMN archived_at TEXT;
