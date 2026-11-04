//! Collection snapshots must preserve real data and never prune unrelated files.

use std::fs::{self, OpenOptions};
use std::path::Path;

use cardfile_core::{Actor, EncryptedPayload, LockMode, NewCard};
use cardfile_db::{BackupKind, ContentUpdate, Database, validate_backup};
use diesel::connection::SimpleConnection;
use diesel::{Connection, SqliteConnection};

const COLLECTION: &str = "019a42ca-1e00-7000-8000-000000000001";
const OTHER_COLLECTION: &str = "019a42ca-1e00-7000-8000-000000000002";

#[allow(clippy::unwrap_used)] // Test fixture helper: an unreadable temp dir must fail the test.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn snapshot_preserves_uncheckpointed_accounts_cards_and_attachment_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("cardfile.db");
    let db = Database::open(&live, None).unwrap();
    let user = db
        .create_user("alice", "Alice", Some("stored-password-hash"), true)
        .unwrap();
    let actor = Actor::user(user.id);
    let blob = db
        .put_blob(
            b"attachment\0binary\xff",
            "application/octet-stream",
            Some("notes.bin"),
        )
        .unwrap();
    let card = db
        .create_card(
            &actor,
            NewCard {
                title: "Draft".into(),
                content: "{}".into(),
                plain_text: "Draft".into(),
                lock_mode: LockMode::None,
            },
        )
        .unwrap();
    db.update_card_content(
        &actor,
        card.id,
        ContentUpdate {
            title: "Last committed edit".into(),
            content: "{}".into(),
            plain_text: "latest content in WAL".into(),
            blob_ids: vec![blob.id.0.clone()],
            ..Default::default()
        },
        10,
        0,
    )
    .unwrap();
    let encrypted = db
        .create_card(
            &actor,
            NewCard {
                title: "Protected".into(),
                content: "{}".into(),
                plain_text: "".into(),
                lock_mode: LockMode::None,
            },
        )
        .unwrap();
    let payload = EncryptedPayload {
        v: 1,
        salt: "sealed-salt".into(),
        nonce: "sealed-nonce".into(),
        m_cost: 1,
        t_cost: 1,
        p_cost: 1,
        ciphertext: "ciphertext-preserved-as-is".into(),
    };
    db.set_card_lock(
        &actor,
        encrypted.id,
        LockMode::Encrypted,
        "",
        "",
        Some(&payload),
        &[],
        &[],
    )
    .unwrap();
    // Encryption checkpoints existing WAL; write once more before the snapshot.
    db.create_user("bob", "Bob", None, false).unwrap();
    assert!(
        fs::metadata(dir.path().join("cardfile.db-wal"))
            .unwrap()
            .len()
            > 0
    );

    let target = db
        .backup_collection(
            &dir.path().join("backups"),
            COLLECTION,
            BackupKind::Manual,
            10,
        )
        .unwrap();
    let copy = Database::open(&target, None).unwrap();
    let restored_user = copy.user(user.id).unwrap();
    assert_eq!(
        restored_user.password_hash.as_deref(),
        Some("stored-password-hash")
    );
    assert!(restored_user.is_admin);
    assert_eq!(copy.card(card.id).unwrap().title, "Last committed edit");
    assert_eq!(
        copy.card(card.id).unwrap().plain_text,
        "latest content in WAL"
    );
    let restored_blob = copy.blob(&blob.id.0).unwrap().unwrap();
    assert_eq!(restored_blob.data, b"attachment\0binary\xff");
    assert_eq!(restored_blob.meta.file_name.as_deref(), Some("notes.bin"));
    assert_eq!(copy.card_revisions(card.id).unwrap().len(), 1);
    let protected = copy.card(encrypted.id).unwrap();
    assert_eq!(protected.lock_mode, LockMode::Encrypted);
    assert_eq!(protected.encrypted, Some(payload));
    assert!(protected.content.is_empty());
    assert_eq!(copy.list_users().unwrap().len(), 2);
    assert!(
        !names(&dir.path().join("backups"))
            .iter()
            .any(|name| name.ends_with(".partial"))
    );
}

#[test]
fn automatic_retention_keeps_manual_safety_and_other_collections() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open_in_memory().unwrap();
    let manual = db
        .backup_collection(dir.path(), COLLECTION, BackupKind::Manual, 1)
        .unwrap();
    let safety = db
        .backup_collection(dir.path(), COLLECTION, BackupKind::BeforeRestore, 1)
        .unwrap();
    let other = db
        .backup_collection(dir.path(), OTHER_COLLECTION, BackupKind::Automatic, 1)
        .unwrap();
    let old = db
        .backup_collection(dir.path(), COLLECTION, BackupKind::Automatic, 1)
        .unwrap();
    fs::write(dir.path().join("unrelated.db"), b"not managed").unwrap();
    let lookalike = dir
        .path()
        .join(format!("cardfile-{COLLECTION}-automatic-important.db"));
    fs::write(&lookalike, b"not a generated snapshot").unwrap();
    let latest = db
        .backup_collection(dir.path(), COLLECTION, BackupKind::Automatic, 1)
        .unwrap();

    assert!(!old.exists());
    for retained in [manual, safety, other, latest] {
        assert!(
            retained.exists(),
            "unexpectedly removed {}",
            retained.display()
        );
        validate_backup(&retained).unwrap();
    }
    assert_eq!(
        fs::read(dir.path().join("unrelated.db")).unwrap(),
        b"not managed"
    );
    assert_eq!(fs::read(lookalike).unwrap(), b"not a generated snapshot");
    assert_eq!(names(dir.path()).len(), 6);
}

#[test]
fn rejects_corrupt_foreign_future_and_missing_backups() {
    let dir = tempfile::tempdir().unwrap();
    let corrupt = dir.path().join("corrupt.db");
    fs::write(&corrupt, b"not a SQLite database").unwrap();
    let foreign = dir.path().join("foreign.db");
    SqliteConnection::establish(&foreign.to_string_lossy())
        .unwrap()
        .batch_execute("CREATE TABLE something_else(id INTEGER);")
        .unwrap();
    let future = dir.path().join("future.db");
    drop(Database::open(&future, None).unwrap());
    SqliteConnection::establish(&future.to_string_lossy())
        .unwrap()
        .batch_execute("INSERT INTO __diesel_schema_migrations(version) VALUES ('99991231000000');")
        .unwrap();

    for invalid in [corrupt, foreign, future, dir.path().join("missing.db")] {
        assert!(
            validate_backup(&invalid).is_err(),
            "accepted {}",
            invalid.display()
        );
    }
}

#[test]
fn restore_freezes_old_handles_and_restarts_with_the_selected_collection() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("cardfile.db");
    let db = Database::open(&live, None).unwrap();
    db.create_user("old", "Old", None, true).unwrap();
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.db");
    {
        let selected = Database::open(&source, None).unwrap();
        selected
            .create_user("restored", "Restored", Some("restored-password"), true)
            .unwrap();
    }
    let original_source = fs::read(&source).unwrap();
    let safety = dir.path().join("safety");
    db.prepare_restore(&source, &safety).unwrap();
    assert!(db.list_users().is_err());
    assert!(db.create_user("too-late", "Too late", None, true).is_err());
    assert!(dir.path().join(".restore-pending").exists());
    assert_eq!(names(&safety).len(), 1);
    let safety_db = Database::open(&safety.join(&names(&safety)[0]), None).unwrap();
    assert_eq!(safety_db.list_users().unwrap()[0].username, "old");
    drop(safety_db);
    drop(db);

    let restored = Database::open(&live, None).unwrap();
    let users = restored.list_users().unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].username, "restored");
    assert_eq!(users[0].password_hash.as_deref(), Some("restored-password"));
    assert!(!dir.path().join(".restore-pending").exists());
    assert!(!dir.path().join(".restore-next.db").exists());
    assert_eq!(fs::read(source).unwrap(), original_source);
}

#[test]
fn restart_recovers_after_either_restore_rename_boundary() {
    for replacement_already_moved in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("cardfile.db");
        {
            let original = Database::open(&live, None).unwrap();
            original.create_user("old", "Old", None, true).unwrap();
        }
        let next = dir.path().join(".restore-next.db");
        {
            let replacement = Database::open(&next, None).unwrap();
            replacement.create_user("new", "New", None, true).unwrap();
        }
        fs::write(dir.path().join(".restore-pending"), b"").unwrap();
        fs::rename(&live, dir.path().join(".restore-previous.db")).unwrap();
        if replacement_already_moved {
            fs::rename(&next, &live).unwrap();
        }

        let recovered = Database::open(&live, None).unwrap();
        assert_eq!(recovered.list_users().unwrap()[0].username, "new");
        assert!(!dir.path().join(".restore-pending").exists());
        assert!(!next.exists());
    }
}

#[test]
fn rejected_restore_keeps_live_database_usable_and_creates_no_pending_job() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("cardfile.db"), None).unwrap();
    db.create_user("retained", "Retained", None, true).unwrap();
    let source = dir.path().join("invalid.db");
    fs::write(&source, b"invalid").unwrap();
    assert!(
        db.prepare_restore(&source, &dir.path().join("safety"))
            .is_err()
    );
    assert_eq!(db.list_users().unwrap()[0].username, "retained");
    assert!(!dir.path().join(".restore-pending").exists());
    assert!(!dir.path().join(".restore-next.db").exists());
    assert!(!dir.path().join("safety").exists());
}

#[test]
fn database_lock_is_held_until_the_last_cloned_handle_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cardfile.db");
    let db = Database::open(&path, None).unwrap();
    let cloned = db.clone();
    let contender = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))
        .unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(db);
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
    drop(cloned);
    contender.try_lock().unwrap();
}

#[test]
fn corrupted_staged_restore_rolls_back_to_the_mandatory_safety_copy() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("cardfile.db");
    let db = Database::open(&live, None).unwrap();
    db.create_user("keep", "Keep", None, true).unwrap();
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.db");
    drop(Database::open(&source, None).unwrap());
    db.prepare_restore(&source, &dir.path().join("backups"))
        .unwrap();
    fs::write(
        dir.path().join(".restore-next.db"),
        b"simulated interrupted/corrupt copy",
    )
    .unwrap();
    drop(db);
    // Report the failed restore once; the original collection is already recovered.
    assert!(Database::open(&live, None).is_err());
    assert!(!dir.path().join(".restore-pending").exists());
    let recovered = Database::open(&live, None).unwrap();
    assert_eq!(recovered.list_users().unwrap()[0].username, "keep");
    assert!(
        names(dir.path())
            .iter()
            .any(|n| n.starts_with(".restore-rejected-"))
    );
}

#[test]
fn uncommitted_restore_staging_is_archived_without_replacing_live_data() {
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("cardfile.db");
    {
        let db = Database::open(&live, None).unwrap();
        db.create_user("keep", "Keep", None, true).unwrap();
    }
    fs::write(dir.path().join(".restore-next.db"), b"unfinished staging").unwrap();
    let recovered = Database::open(&live, None).unwrap();
    assert_eq!(recovered.list_users().unwrap()[0].username, "keep");
    assert!(!dir.path().join(".restore-next.db").exists());
    let archived = names(dir.path())
        .into_iter()
        .find(|n| n.starts_with(".restore-abandoned-"))
        .unwrap();
    assert_eq!(
        fs::read(dir.path().join(archived)).unwrap(),
        b"unfinished staging"
    );
}

#[test]
fn restores_supported_older_schema_without_migrating_the_selected_source() {
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
    const TEST_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    let dir = tempfile::tempdir().unwrap();
    let live = dir.path().join("cardfile.db");
    let db = Database::open(&live, None).unwrap();
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.db");
    {
        let selected = Database::open(&source, None).unwrap();
        selected.create_user("older", "Older", None, true).unwrap();
    }
    {
        let mut conn = SqliteConnection::establish(&source.to_string_lossy()).unwrap();
        conn.revert_last_migration(TEST_MIGRATIONS).unwrap();
        conn.revert_last_migration(TEST_MIGRATIONS).unwrap();
    }
    let bytes = fs::read(&source).unwrap();
    db.prepare_restore(&source, &dir.path().join("backups"))
        .unwrap();
    drop(db);
    let restored = Database::open(&live, None).unwrap();
    assert_eq!(restored.list_users().unwrap()[0].username, "older");
    assert!(restored.app_settings().is_ok());
    assert_eq!(fs::read(source).unwrap(), bytes);
}
