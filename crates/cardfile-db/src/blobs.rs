use cardfile_core::{Blob, BlobId, BlobMeta, now_millis};
use diesel::prelude::*;

use crate::schema::{blobs, card_blobs};
use crate::{Database, DbResult};

/// `(id, mime, size, file_name, data)`.
type BlobRow = (String, String, i64, Option<String>, Vec<u8>);

impl Database {
    /// Store bytes (deduplicated by BLAKE3 hash) and return their metadata.
    pub fn put_blob(&self, data: &[u8], mime: &str, file_name: Option<&str>) -> DbResult<BlobMeta> {
        let id = blake3::hash(data).to_hex().to_string();
        let size = i64::try_from(data.len()).unwrap_or(i64::MAX);
        self.with(|conn| {
            diesel::insert_or_ignore_into(blobs::table)
                .values((
                    blobs::id.eq(&id),
                    blobs::mime.eq(mime),
                    blobs::size.eq(size),
                    blobs::file_name.eq(file_name),
                    blobs::data.eq(data),
                    blobs::created_at.eq(now_millis()),
                ))
                .execute(conn)?;
            Ok(BlobMeta {
                id: BlobId(id),
                mime: mime.to_owned(),
                size,
                file_name: file_name.map(str::to_owned),
            })
        })
    }

    /// Store a blob sealed with [`cardfile_crypto`]-style per-blob encryption under
    /// `id` (see [`cardfile_core::BlobRef`]). Name and type stay in the encrypted
    /// document, so the row only reveals the size.
    pub fn put_sealed_blob(&self, id: &str, sealed: &[u8]) -> DbResult<()> {
        let size = i64::try_from(sealed.len()).unwrap_or(i64::MAX);
        self.with(|conn| {
            diesel::insert_into(blobs::table)
                .values((
                    blobs::id.eq(id),
                    blobs::mime.eq("application/octet-stream"),
                    blobs::size.eq(size),
                    blobs::file_name.eq(None::<String>),
                    blobs::data.eq(sealed),
                    blobs::created_at.eq(now_millis()),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn blob(&self, id: &str) -> DbResult<Option<Blob>> {
        self.with(|conn| {
            let row: Option<BlobRow> = blobs::table
                .find(id)
                .select((
                    blobs::id,
                    blobs::mime,
                    blobs::size,
                    blobs::file_name,
                    blobs::data,
                ))
                .first(conn)
                .optional()?;
            Ok(row.map(|(id, mime, size, file_name, data)| Blob {
                meta: BlobMeta {
                    id: BlobId(id),
                    mime,
                    size,
                    file_name,
                },
                data,
            }))
        })
    }

    /// Remove blobs no card references. Returns how many were deleted.
    pub fn collect_garbage_blobs(&self) -> DbResult<usize> {
        self.with(gc)
    }
}

/// Unlink every blob from a card except `keep` and delete the unlinked ones no other
/// card uses, without the grace period: used when a card becomes encrypted so its
/// plaintext images go (its sealed ones are in `keep`).
pub(crate) fn unlink_card_blobs(
    conn: &mut SqliteConnection,
    card_id: &str,
    keep: &[&str],
) -> DbResult<()> {
    let linked: Vec<String> = card_blobs::table
        .filter(card_blobs::card_id.eq(card_id))
        .filter(card_blobs::blob_id.ne_all(keep))
        .select(card_blobs::blob_id)
        .load(conn)?;
    diesel::delete(
        card_blobs::table
            .filter(card_blobs::card_id.eq(card_id))
            .filter(card_blobs::blob_id.eq_any(&linked)),
    )
    .execute(conn)?;
    let still_used = card_blobs::table.select(card_blobs::blob_id);
    diesel::delete(
        blobs::table
            .filter(blobs::id.eq_any(&linked))
            .filter(blobs::id.ne_all(still_used)),
    )
    .execute(conn)?;
    Ok(())
}

/// Delete blobs unreferenced by any card, keeping ones added in the last hour (an
/// editor may hold a freshly pasted image that is not saved yet).
pub(crate) fn gc(conn: &mut SqliteConnection) -> DbResult<usize> {
    let cutoff = now_millis() - 3_600_000;
    let referenced = card_blobs::table.select(card_blobs::blob_id);
    let n = diesel::delete(
        blobs::table
            .filter(blobs::id.ne_all(referenced))
            .filter(blobs::created_at.lt(cutoff)),
    )
    .execute(conn)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use crate::test_util::*;

    #[test]
    fn blobs_are_deduplicated() {
        let db = db();
        let a = db
            .put_blob(b"png bytes", "image/png", Some("a.png"))
            .unwrap();
        let b = db
            .put_blob(b"png bytes", "image/png", Some("b.png"))
            .unwrap();
        assert_eq!(a.id, b.id);
        let blob = db.blob(&a.id.0).unwrap().unwrap();
        assert_eq!(blob.data, b"png bytes");
        assert_eq!(blob.meta.file_name.as_deref(), Some("a.png"));
        assert!(db.blob("missing").unwrap().is_none());
        // Fresh blobs survive garbage collection.
        assert_eq!(db.collect_garbage_blobs().unwrap(), 0);
    }
}
