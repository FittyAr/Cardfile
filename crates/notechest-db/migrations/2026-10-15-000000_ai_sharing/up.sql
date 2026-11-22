-- Per-user lock keys, card sharing, AI access through API keys and attribution.

-- Accounts: the AI assistant is a user of kind 'ai'. `lock_hash` is a lock password
-- separate from the sign-in password (NULL = use the sign-in password). The X25519
-- key pair lets encrypted cards be shared: `secret_key` is sealed under the lock
-- password.
ALTER TABLE users ADD COLUMN kind TEXT NOT NULL DEFAULT 'human';
ALTER TABLE users ADD COLUMN lock_hash TEXT;
ALTER TABLE users ADD COLUMN public_key TEXT;
ALTER TABLE users ADD COLUMN secret_key TEXT;
-- Password-free installs kept the lock password in the user settings.
UPDATE users SET lock_hash = json_extract(settings, '$.lock_password_hash')
WHERE json_valid(settings) AND json_extract(settings, '$.lock_password_hash') IS NOT NULL;

-- Attribution: who made the last change, and who wrote each snapshotted version.
ALTER TABLE cards ADD COLUMN updated_by TEXT;
UPDATE cards SET updated_by = owner_id;
ALTER TABLE card_revisions ADD COLUMN author_id TEXT;

-- Cards shared with other users ('view' or 'edit').
CREATE TABLE card_shares (
    card_id    TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users (id),
    permission TEXT NOT NULL DEFAULT 'view',
    created_at BIGINT NOT NULL,
    PRIMARY KEY (card_id, user_id)
);
CREATE INDEX card_shares_user ON card_shares (user_id);

-- The random key of an encrypted card (format v2), wrapped for each user with access.
CREATE TABLE card_keys (
    card_id    TEXT NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    user_id    TEXT NOT NULL REFERENCES users (id),
    wrapped    TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (card_id, user_id)
);

-- Keys AI clients use to act on behalf of a user. Only a BLAKE3 hash is stored.
CREATE TABLE api_keys (
    id            TEXT PRIMARY KEY NOT NULL,
    user_id       TEXT NOT NULL REFERENCES users (id),
    name          TEXT NOT NULL,
    prefix        TEXT NOT NULL,
    key_hash      TEXT NOT NULL UNIQUE,
    allow_private BOOLEAN NOT NULL DEFAULT 0,
    read_only     BOOLEAN NOT NULL DEFAULT 0,
    created_at    BIGINT NOT NULL,
    last_used_at  BIGINT,
    revoked_at    BIGINT
);
CREATE INDEX api_keys_user ON api_keys (user_id);

-- Append-only log of changes, including the AI's.
CREATE TABLE activity (
    id           TEXT PRIMARY KEY NOT NULL,
    card_id      TEXT,
    actor_id     TEXT NOT NULL,
    on_behalf_of TEXT,
    via          TEXT,
    action       TEXT NOT NULL,
    summary      TEXT NOT NULL DEFAULT '',
    created_at   BIGINT NOT NULL
);
CREATE INDEX activity_card ON activity (card_id, created_at);
CREATE INDEX activity_principal ON activity (on_behalf_of, created_at);
