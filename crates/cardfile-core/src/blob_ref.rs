//! References from documents to stored blobs.
//!
//! A plain reference is the blob id (BLAKE3 hex of the bytes). Blobs of encrypted
//! cards are sealed with their own random key, which travels inside the (encrypted)
//! document: `sealed:<blob id>:<key>`. Changing the card password therefore never
//! touches the blobs, and the `blobs` table holds nothing readable for them.

const SEALED_PREFIX: &str = "sealed:";

/// A parsed blob reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlobRef<'a> {
    /// Primary key in the `blobs` table.
    pub id: &'a str,
    /// Base64url key of a sealed blob.
    pub key: Option<&'a str>,
}

impl<'a> BlobRef<'a> {
    pub fn parse(reference: &'a str) -> Self {
        reference
            .strip_prefix(SEALED_PREFIX)
            .and_then(|rest| rest.split_once(':'))
            .map_or(
                Self {
                    id: reference,
                    key: None,
                },
                |(id, key)| Self { id, key: Some(key) },
            )
    }

    /// A fresh primary key for a sealed blob. Random rather than a content hash, so
    /// equal files in different encrypted cards cannot be matched up.
    pub fn new_sealed_id() -> String {
        uuid::Uuid::now_v7().simple().to_string()
    }

    /// Build the reference of a sealed blob.
    pub fn sealed(id: &str, key: &str) -> String {
        format!("{SEALED_PREFIX}{id}:{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_sealed_refs() {
        assert_eq!(
            BlobRef::parse("abc"),
            BlobRef {
                id: "abc",
                key: None
            }
        );
        let r = BlobRef::sealed("0193", "k-y_");
        assert_eq!(
            BlobRef::parse(&r),
            BlobRef {
                id: "0193",
                key: Some("k-y_")
            }
        );
    }
}
