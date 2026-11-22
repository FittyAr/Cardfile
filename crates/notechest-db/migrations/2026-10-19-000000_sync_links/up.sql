-- Sync-ready links: shares and card tags are soft-deleted (`deleted_at`) and carry
-- `rev`/`updated_at` like every other synced row, instead of being hard-deleted.
ALTER TABLE card_shares ADD COLUMN updated_at BIGINT NOT NULL DEFAULT 0;
ALTER TABLE card_shares ADD COLUMN deleted_at BIGINT;
ALTER TABLE card_shares ADD COLUMN rev BIGINT NOT NULL DEFAULT 1;
UPDATE card_shares SET updated_at = created_at;

ALTER TABLE card_tags ADD COLUMN created_at BIGINT NOT NULL DEFAULT 0;
ALTER TABLE card_tags ADD COLUMN updated_at BIGINT NOT NULL DEFAULT 0;
ALTER TABLE card_tags ADD COLUMN deleted_at BIGINT;
ALTER TABLE card_tags ADD COLUMN rev BIGINT NOT NULL DEFAULT 1;
UPDATE card_tags SET
    created_at = (SELECT c.updated_at FROM cards c WHERE c.id = card_tags.card_id),
    updated_at = (SELECT c.updated_at FROM cards c WHERE c.id = card_tags.card_id);

-- Remove deleted entries from the full-text index right away instead of leaving
-- them in older segments until a merge: text a card had before it was encrypted
-- must not stay recoverable from the index.
INSERT INTO cards_fts (cards_fts, rank) VALUES ('secure-delete', 1);
