-- Kundenverwaltung: Personen werden zu Kunden mit Kundennummer und Kontaktdaten.
-- Jeder Auftrag braucht einen Kunden (prüft die App beim Anlegen und Ändern).
ALTER TABLE people ADD COLUMN number BIGINT;
ALTER TABLE people ADD COLUMN email TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN phone TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN address TEXT NOT NULL DEFAULT '';

-- Kundennummern in Anlage-Reihenfolge vergeben
UPDATE people SET number = (
    SELECT COUNT(*) FROM people p2
    WHERE p2.created_at < people.created_at OR (p2.created_at = people.created_at AND p2.id <= people.id)
);

-- Bisheriges Feld „Kontakt“ aufteilen: mit @ als E-Mail, sonst als Telefon
UPDATE people SET email = contact WHERE contact LIKE '%@%';
UPDATE people SET phone = contact WHERE contact <> '' AND contact NOT LIKE '%@%';

CREATE UNIQUE INDEX people_number ON people (number);
