-- Cardfile 2 initial schema.
-- All ids are UUIDv7 text, timestamps are Unix milliseconds. Rows are soft-deleted
-- (`deleted_at`) and carry `rev` so a future sync engine can reconcile changes.

CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE users (
    id            TEXT PRIMARY KEY NOT NULL,
    username      TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    password_hash TEXT,
    is_admin      BOOLEAN NOT NULL DEFAULT 0,
    settings      TEXT NOT NULL DEFAULT '{}',
    created_at    BIGINT NOT NULL,
    updated_at    BIGINT NOT NULL,
    deleted_at    BIGINT,
    rev           BIGINT NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX users_username ON users (username COLLATE NOCASE) WHERE deleted_at IS NULL;

CREATE TABLE cards (
    id          TEXT PRIMARY KEY NOT NULL,
    owner_id    TEXT NOT NULL REFERENCES users (id),
    title       TEXT NOT NULL,
    content     TEXT NOT NULL DEFAULT '',
    plain_text  TEXT NOT NULL DEFAULT '',
    lock_mode   TEXT NOT NULL DEFAULT 'none',
    encrypted   TEXT,
    is_pinned   BOOLEAN NOT NULL DEFAULT 0,
    is_favorite BOOLEAN NOT NULL DEFAULT 0,
    sort_key    TEXT NOT NULL,
    created_at  BIGINT NOT NULL,
    updated_at  BIGINT NOT NULL,
    deleted_at  BIGINT,
    rev         BIGINT NOT NULL DEFAULT 1
);
CREATE INDEX cards_owner ON cards (owner_id, deleted_at, sort_key);

CREATE TABLE card_revisions (
    id         TEXT PRIMARY KEY NOT NULL,
    card_id    TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    content    TEXT NOT NULL,
    encrypted  TEXT,
    created_at BIGINT NOT NULL
);
CREATE INDEX card_revisions_card ON card_revisions (card_id, created_at);

CREATE TABLE tags (
    id         TEXT PRIMARY KEY NOT NULL,
    owner_id   TEXT NOT NULL REFERENCES users (id),
    name       TEXT NOT NULL,
    color      TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    deleted_at BIGINT,
    rev        BIGINT NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX tags_owner_name ON tags (owner_id, name COLLATE NOCASE) WHERE deleted_at IS NULL;

CREATE TABLE card_tags (
    card_id TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, tag_id)
);
CREATE INDEX card_tags_tag ON card_tags (tag_id);

CREATE TABLE blobs (
    id         TEXT PRIMARY KEY NOT NULL,
    mime       TEXT NOT NULL,
    size       BIGINT NOT NULL,
    file_name  TEXT,
    data       BLOB NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE card_blobs (
    card_id TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    blob_id TEXT NOT NULL REFERENCES blobs (id),
    PRIMARY KEY (card_id, blob_id)
);
CREATE INDEX card_blobs_blob ON card_blobs (blob_id);

-- Full-text search. `tags` holds the space-separated tag names of the card.
-- Content of private/encrypted cards is never indexed.
-- Rows link to cards by `card_id`, not rowid: `cards` has a TEXT primary key, so its
-- implicit rowid may be renumbered by VACUUM.
CREATE VIRTUAL TABLE cards_fts USING fts5 (
    card_id UNINDEXED,
    title,
    body,
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER cards_fts_insert AFTER INSERT ON cards BEGIN
    INSERT INTO cards_fts (card_id, title, body, tags)
    VALUES (
        NEW.id,
        NEW.title,
        CASE WHEN NEW.lock_mode IN ('private', 'encrypted') THEN '' ELSE NEW.plain_text END,
        ''
    );
END;

CREATE TRIGGER cards_fts_update AFTER UPDATE OF title, plain_text, lock_mode ON cards BEGIN
    UPDATE cards_fts SET
        title = NEW.title,
        body = CASE WHEN NEW.lock_mode IN ('private', 'encrypted') THEN '' ELSE NEW.plain_text END
    WHERE card_id = NEW.id;
END;

CREATE TRIGGER cards_fts_delete AFTER DELETE ON cards BEGIN
    DELETE FROM cards_fts WHERE card_id = OLD.id;
END;
