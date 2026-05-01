CREATE TABLE users (
    uuid  TEXT NOT NULL PRIMARY KEY,
    name  TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT ''
);

CREATE TABLE slots (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    label        TEXT    NOT NULL,
    max_bookings INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE bookings (
    user_uuid TEXT    NOT NULL PRIMARY KEY REFERENCES users(uuid),
    slot_id   INTEGER NOT NULL REFERENCES slots(id)
);

CREATE TABLE admins (
    uuid TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL
);
