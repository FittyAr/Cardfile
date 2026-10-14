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
