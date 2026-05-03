ALTER TABLE bookings ADD COLUMN name  TEXT NOT NULL DEFAULT '';
ALTER TABLE bookings ADD COLUMN email TEXT NOT NULL DEFAULT '';

-- copy existing name/email into bookings
UPDATE bookings SET
    name  = (SELECT name  FROM users WHERE users.uuid = bookings.user_uuid),
    email = (SELECT email FROM users WHERE users.uuid = bookings.user_uuid);

-- recreate users without name/email (DROP COLUMN requires SQLite 3.35+)
CREATE TABLE users_new (
    uuid TEXT NOT NULL PRIMARY KEY
);
INSERT INTO users_new SELECT uuid FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;
