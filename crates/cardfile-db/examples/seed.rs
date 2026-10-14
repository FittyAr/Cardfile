//! Create a demo database: `cargo run -p cardfile-db --example seed -- <data-dir>`.
//! Then start Cardfile with `--data-dir <data-dir>`.

use cardfile_core::{AppSettings, AuthMode, LockMode, NewCard, UserSettings};
use cardfile_db::Database;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: seed <data-dir>")?;
    let path = std::path::Path::new(&dir).join("cardfile.db");
    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }
    let db = Database::open(&path, None)?;
    db.save_app_settings(&AppSettings {
        initialized: true,
        auth_mode: AuthMode::NoPassword,
        ..Default::default()
    })?;
    let user = db.create_user("demo", "Demo User", None, true)?;
    let settings = UserSettings {
        theme: "Midnight".into(),
        ..Default::default()
    };
    db.save_user_settings(user.id, &settings)?;
    let work = db.create_tag(user.id, "Work", "#3b82f6")?;
    let ideas = db.create_tag(user.id, "Ideas", "#22c55e")?;
    let recipes = db.create_tag(user.id, "Recipes", "#f97316")?;
    let cards: &[(&str, &str, &[cardfile_core::TagId], LockMode)] = &[
        (
            "Quarterly plan",
            "# Goals\n\n- Ship **Cardfile 2.0**\n- Write the user guide\n- [x] Pick GPUI\n- [ ] Publish to winget\n\n| Area | Owner |\n|---|---|\n| Editor | Ana |\n| Storage | Luis |",
            &[work.id],
            LockMode::None,
        ),
        (
            "Pancakes",
            "## Ingredients\n\n1. 2 eggs\n2. 250 ml milk\n3. 125 g flour\n\n> Rest the batter for 30 minutes.",
            &[recipes.id],
            LockMode::ReadOnly,
        ),
        (
            "App ideas",
            "A *habit tracker* with streaks.\n\nA tool to `diff` recipes.",
            &[ideas.id],
            LockMode::None,
        ),
        (
            "Meeting notes",
            "Discussed the release checklist and the **Microsoft Store** submission.",
            &[work.id],
            LockMode::None,
        ),
        (
            "Bank details",
            "Account numbers live here.",
            &[],
            LockMode::Private,
        ),
    ];
    for (title, md, tags, lock) in cards {
        let doc = cardfile_doc::from_markdown(md);
        let card = db.create_card(
            user.id,
            NewCard {
                title: (*title).into(),
                content: doc.to_json(),
                plain_text: cardfile_doc::to_plain_text(&doc),
                lock_mode: *lock,
            },
        )?;
        db.set_card_tags(card.id, tags)?;
        if *title == "Quarterly plan" {
            db.set_card_pinned(card.id, true)?;
            db.set_card_favorite(card.id, true)?;
        }
    }
    println!("seeded {}", path.display());
    Ok(())
}
