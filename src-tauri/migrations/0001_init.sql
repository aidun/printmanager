-- Portables SQL: läuft unter SQLite und PostgreSQL.
-- IDs sind UUID-Text, Zeitpunkte ISO-8601-Text, Ganzzahlen BIGINT, Wahrheitswerte 0/1.

CREATE TABLE people (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    contact     TEXT NOT NULL DEFAULT '',
    note        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL
);

CREATE TABLE orders (
    id          TEXT PRIMARY KEY,
    number      BIGINT NOT NULL UNIQUE,
    person_id   TEXT REFERENCES people(id),
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL,
    priority    BIGINT NOT NULL DEFAULT 0,
    due_date    TEXT,
    position    BIGINT NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX orders_status ON orders (status, position);

CREATE TABLE order_items (
    id          TEXT PRIMARY KEY,
    order_id    TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    position    BIGINT NOT NULL DEFAULT 0,
    title       TEXT NOT NULL,
    quantity    BIGINT NOT NULL DEFAULT 1,
    color_wish  TEXT NOT NULL DEFAULT '',
    note        TEXT NOT NULL DEFAULT '',
    catalog_id  TEXT,
    done        BIGINT NOT NULL DEFAULT 0
);
CREATE INDEX order_items_order ON order_items (order_id, position);

CREATE TABLE attachments (
    id          TEXT PRIMARY KEY,
    order_id    TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,          -- image | file | link | text
    name        TEXT NOT NULL DEFAULT '',
    path        TEXT,                   -- relativ zum Auftragsordner
    url         TEXT,
    body        TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX attachments_order ON attachments (order_id, created_at);

CREATE TABLE events (
    id          TEXT PRIMARY KEY,
    order_id    TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,          -- created | status | note | attachment | edit
    from_status TEXT,
    to_status   TEXT,
    body        TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL
);
CREATE INDEX events_order ON events (order_id, created_at);
