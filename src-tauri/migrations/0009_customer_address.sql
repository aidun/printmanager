-- Kundenadresse aufgeteilt (Straße, Hausnummer, PLZ, Ort) und optionale abweichende Rechnungsadresse.
ALTER TABLE people ADD COLUMN street TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN house_no TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN zip TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN city TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN billing_differs BIGINT NOT NULL DEFAULT 0;
ALTER TABLE people ADD COLUMN billing_name TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN billing_street TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN billing_house_no TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN billing_zip TEXT NOT NULL DEFAULT '';
ALTER TABLE people ADD COLUMN billing_city TEXT NOT NULL DEFAULT '';

-- Bisherige Freitext-Adresse bleibt erhalten und steht vorerst in „Straße“, bis sie aufgeteilt wird
UPDATE people SET street = address WHERE address <> '';
