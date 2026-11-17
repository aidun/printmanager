-- Von Hand festgelegte Zuordnung eines Druckauftrags (Name vom Drucker, vereinheitlicht) zu einem Auftrag.
CREATE TABLE print_assignments (
    task_key   TEXT PRIMARY KEY,
    order_id   TEXT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);
