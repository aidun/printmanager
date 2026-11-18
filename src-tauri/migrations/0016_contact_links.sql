-- Abgleich verknüpfter Kontakte (docs/PLAN-Anbindungen.md, K1/K2): letzter gemeinsamer Stand je Seite für den
-- Vergleich je Feld, Zustand der Verknüpfung und Felder im Konflikt.
ALTER TABLE external_links ADD COLUMN remote_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE external_links ADD COLUMN local_snapshot TEXT NOT NULL DEFAULT '';
ALTER TABLE external_links ADD COLUMN state TEXT NOT NULL DEFAULT 'ok';
ALTER TABLE external_links ADD COLUMN conflict TEXT NOT NULL DEFAULT '';
