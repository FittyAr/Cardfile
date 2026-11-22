//! Collections written under the previous name (Cardfile) must be adopted by
//! NoteChest without losing or mixing data.

use std::fs;
use std::path::Path;

use diesel::connection::SimpleConnection;
use diesel::sql_types::BigInt;
use diesel::{Connection, RunQueryDsl, SqliteConnection};
use notechest_core::{Actor, NewCard};
use notechest_db::{BackupKind, Database, DbError, has_collection};

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
fn a_collection_from_before_the_rename_is_adopted_under_the_new_name() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("cardfile.db");
    let card_id = {
        let db = Database::open(&legacy, None).unwrap();
        let user = db
            .create_user("alice", "Alice", Some("stored-password-hash"), true)
            .unwrap();
        db.create_card(
            &Actor::user(user.id),
            NewCard {
                title: "Kept".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .id
    };
    assert!(has_collection(dir.path()));

    let live = dir.path().join("notechest.db");
    let db = Database::open(&live, None).unwrap();

    assert_eq!(db.card(card_id).unwrap().title, "Kept");
    assert!(db.user_by_username("alice").unwrap().is_some());
    let files = names(dir.path());
    assert!(!files.contains(&"cardfile.db".to_owned()), "{files:?}");
    assert!(!files.contains(&"cardfile.lock".to_owned()), "{files:?}");
    assert!(files.contains(&"notechest.db".to_owned()), "{files:?}");
}

#[test]
fn changes_still_in_the_write_ahead_log_survive_the_rename() {
    let origin = tempfile::tempdir().unwrap();
    let source = origin.path().join("cardfile.db");
    // A session that ended without folding its log into the file (a crash): the
    // rows exist only in `cardfile.db-wal`.
    let session = SqliteConnection::establish(&source.to_string_lossy()).unwrap();
    let mut session = session;
    session
        .batch_execute(
            "PRAGMA journal_mode = WAL;
             PRAGMA wal_autocheckpoint = 0;
             CREATE TABLE unsaved (n INTEGER);
             INSERT INTO unsaved VALUES (41), (42);",
        )
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    fs::copy(&source, dir.path().join("cardfile.db")).unwrap();
    fs::copy(
        origin.path().join("cardfile.db-wal"),
        dir.path().join("cardfile.db-wal"),
    )
    .unwrap();
    drop(session);

    let live = dir.path().join("notechest.db");
    drop(Database::open(&live, None).unwrap());

    let mut conn = SqliteConnection::establish(&live.to_string_lossy()).unwrap();
    let count: i64 = diesel::select(diesel::dsl::sql::<BigInt>("(SELECT count(*) FROM unsaved)"))
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(count, 2, "rows that lived only in the old WAL were lost");
    assert!(!dir.path().join("cardfile.db").exists());
    assert!(!dir.path().join("cardfile.db-wal").exists());
}

#[test]
fn a_collection_still_open_in_the_previous_version_is_not_touched() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("cardfile.db");
    let running = Database::open(&legacy, None).unwrap();
    running.create_user("alice", "Alice", None, true).unwrap();

    let live = dir.path().join("notechest.db");
    let err = Database::open(&live, None).unwrap_err();

    assert!(matches!(err, DbError::AlreadyOpen), "{err:?}");
    assert!(legacy.is_file());
    assert!(!live.exists());
    assert!(running.user_by_username("alice").unwrap().is_some());
}

#[test]
fn an_existing_new_database_wins_over_a_leftover_old_one() {
    let dir = tempfile::tempdir().unwrap();
    // The new database exists first (a fresh folder); an old file shows up later.
    {
        let db = Database::open(&dir.path().join("notechest.db"), None).unwrap();
        db.create_user("current", "Current", None, true).unwrap();
    }
    {
        let db = Database::open(&dir.path().join("cardfile.db"), None).unwrap();
        db.create_user("old", "Old", None, true).unwrap();
    }
    // Neither file is replaced or merged.
    let db = Database::open(&dir.path().join("notechest.db"), None).unwrap();
    assert!(db.user_by_username("current").unwrap().is_some());
    assert!(db.user_by_username("old").unwrap().is_none());
    assert!(dir.path().join("cardfile.db").is_file());
}

#[test]
fn a_fresh_folder_just_gets_the_new_name() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!has_collection(dir.path()));
    drop(Database::open(&dir.path().join("notechest.db"), None).unwrap());
    assert!(has_collection(dir.path()));
    assert!(!dir.path().join("cardfile.db").exists());
}

#[test]
fn old_and_new_automatic_backups_share_one_retention() {
    let dir = tempfile::tempdir().unwrap();
    let backups = dir.path().join("backups");
    fs::create_dir_all(&backups).unwrap();
    let db = Database::open(&dir.path().join("notechest.db"), None).unwrap();
    let id = |n: u8| format!("019a42ca-1e00-7000-8000-0000000000{n:02}");
    // Two copies from before the rename, one of another collection, one stray file.
    let old_a = format!("cardfile-{COLLECTION}-automatic-1000-{}.db", id(10));
    let old_b = format!("cardfile-{COLLECTION}-automatic-2000-{}.db", id(11));
    let other = format!("cardfile-{OTHER_COLLECTION}-automatic-1500-{}.db", id(12));
    for name in [&old_a, &old_b, &other, &"notes.db".to_owned()] {
        fs::write(backups.join(name), b"not a database").unwrap();
    }

    db.backup_collection(&backups, COLLECTION, BackupKind::Automatic, 2)
        .unwrap();

    let files = names(&backups);
    assert!(!files.contains(&old_a), "oldest copy kept: {files:?}");
    assert!(files.contains(&old_b), "{files:?}");
    assert!(files.contains(&other), "another collection was pruned");
    assert!(files.contains(&"notes.db".to_owned()), "stray file removed");
    let fresh: Vec<_> = files
        .iter()
        .filter(|n| n.starts_with(&format!("notechest-{COLLECTION}-automatic-")))
        .collect();
    assert_eq!(fresh.len(), 1, "{files:?}");
}
