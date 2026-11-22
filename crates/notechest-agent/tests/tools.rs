//! End-to-end checks of the AI tools against an in-memory database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use notechest_agent::{Scope, call, tools};
use notechest_core::{Actor, CardFilter, CardId, NewCard};
use notechest_db::{CardMove, Database};
use serde_json::{Value, json};

fn setup(read_only: bool) -> (Database, Scope) {
    let db = Database::open_in_memory().unwrap();
    let user = db.create_user("ana", "Ana", None, true).unwrap();
    let ai = db.ai_user().unwrap();
    let scope = Scope {
        principal: user.id,
        ai: ai.id,
        via: "test".into(),
        allow_private: false,
        read_only,
    };
    (db, scope)
}

fn card(db: &Database, scope: &Scope, title: &str) -> CardId {
    db.create_card(
        &Actor::user(scope.principal),
        NewCard {
            title: title.into(),
            ..Default::default()
        },
    )
    .unwrap()
    .id
}

fn titles(db: &Database, scope: &Scope) -> Vec<String> {
    db.list_cards(scope.principal, &CardFilter::default())
        .unwrap()
        .into_iter()
        .map(|c| c.title)
        .collect()
}

fn run(db: &Database, scope: &Scope, name: &str, args: Value) -> Result<Value, String> {
    call(db, scope, name, &args).map(|o| o.value)
}

#[test]
fn read_only_scope_offers_and_runs_only_reading_tools() {
    let (db, scope) = setup(true);
    let id = card(&db, &scope, "Notes");
    assert!(tools(&scope).iter().all(|t| t.read_only));
    assert!(run(&db, &scope, "search_cards", json!({})).is_ok());
    assert!(run(&db, &scope, "get_card", json!({"id": id.to_string()})).is_ok());
    for (tool, args) in [
        ("create_card", json!({"title": "x"})),
        ("update_card", json!({"id": id.to_string(), "title": "x"})),
        ("delete_card", json!({"id": id.to_string()})),
    ] {
        let e = run(&db, &scope, tool, args).unwrap_err();
        assert!(e.contains("read-only"), "{tool}: {e}");
    }
    let card = db.card(id).unwrap();
    assert_eq!(card.title, "Notes");
    assert!(!card.is_deleted());
}

#[test]
fn delete_goes_to_the_recycle_bin_and_can_be_restored() {
    let (db, scope) = setup(false);
    let id = card(&db, &scope, "Notes");
    let out = call(&db, &scope, "delete_card", &json!({"id": id.to_string()})).unwrap();
    assert!(out.changed);
    // Soft delete: the row is still there, just flagged.
    assert!(db.card(id).unwrap().is_deleted());
    assert!(titles(&db, &scope).is_empty());
    let bin = run(&db, &scope, "search_cards", json!({"in_recycle_bin": true})).unwrap();
    assert_eq!(bin["total"], 1);
    run(&db, &scope, "restore_card", json!({"id": id.to_string()})).unwrap();
    assert_eq!(titles(&db, &scope), ["Notes"]);
}

#[test]
fn writes_that_remove_or_overwrite_are_flagged_destructive() {
    let (_, scope) = setup(false);
    let destructive: Vec<&str> = tools(&scope)
        .into_iter()
        .filter(|t| t.destructive)
        .map(|t| t.name)
        .collect();
    for name in [
        "delete_card",
        "update_card",
        "edit_card_text",
        "tag_card",
        "reorder_cards",
    ] {
        assert!(destructive.contains(&name), "{name} should be destructive");
    }
    assert!(
        tools(&scope)
            .iter()
            .all(|t| !(t.read_only && t.destructive))
    );
}

#[test]
fn reorder_is_atomic() {
    let (db, scope) = setup(false);
    let ids: Vec<CardId> = ["a", "b", "c", "d"]
        .iter()
        .map(|t| card(&db, &scope, t))
        .collect();
    assert_eq!(titles(&db, &scope), ["d", "c", "b", "a"]);
    let ask: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
    run(&db, &scope, "reorder_cards", json!({"ids": ask})).unwrap();
    assert_eq!(titles(&db, &scope), ["a", "b", "c", "d"]);

    // A batch whose last move fails changes nothing.
    let before = titles(&db, &scope);
    let moves = [
        CardMove {
            id: ids[3],
            before: None,
            after: Some(ids[0]),
        },
        CardMove {
            id: ids[2],
            before: Some(CardId::new()),
            after: None,
        },
    ];
    assert!(
        db.move_cards(&Actor::user(scope.principal), &moves)
            .is_err()
    );
    assert_eq!(titles(&db, &scope), before);

    // A card that is not the user's own aborts the tool before anything moves.
    let other = db.create_user("bo", "Bo", None, false).unwrap();
    let foreign = db
        .create_card(
            &Actor::user(other.id),
            NewCard {
                title: "theirs".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    let ask = json!({"ids": [ids[3].to_string(), ids[0].to_string(), foreign.to_string()]});
    assert!(run(&db, &scope, "reorder_cards", ask).is_err());
    assert_eq!(titles(&db, &scope), before);
}
