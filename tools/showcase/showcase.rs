//! Seed a demo database with fictional cards for the website screenshots:
//! `cargo run -p cardfile-db --example showcase -- <data-dir> <images-dir>`.

use cardfile_core::{Actor, AppSettings, AuthMode, LockMode, NewCard, TagId, UserSettings};
use cardfile_db::{ContentUpdate, Database};
use cardfile_doc::{Block, BlockKind, Document, Image};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

struct Seed<'a> {
    db: &'a Database,
    actor: Actor,
    images: std::path::PathBuf,
}

impl Seed<'_> {
    /// `md` may contain lines `@img:<file>|<width>` replaced by image blocks.
    fn doc(&self, md: &str) -> Res<(Document, Vec<String>)> {
        let mut blocks = Vec::new();
        let mut blobs = Vec::new();
        let mut buf = String::new();
        for line in md.lines().chain(std::iter::once("@img:")) {
            let Some(line) = line.strip_prefix("@img:") else {
                buf.push_str(line);
                buf.push('\n');
                continue;
            };
            if !buf.trim().is_empty() {
                blocks.extend(cardfile_doc::from_markdown(&buf).blocks);
            }
            buf.clear();
            if !line.is_empty() {
                let (file, width) = line.split_once('|').unwrap_or((line, ""));
                let data = std::fs::read(self.images.join(file))?;
                let mime = if file.ends_with(".png") { "image/png" } else { "image/jpeg" };
                let meta = self.db.put_blob(&data, mime, Some(file))?;
                blobs.push(meta.id.0.clone());
                blocks.push(Block::new(
                    BlockKind::Image(Image {
                        blob: meta.id.0,
                        mime: mime.into(),
                        width: width.parse().ok(),
                        height: None,
                        alt: String::new(),
                    }),
                    "",
                ));
            }
        }
        let mut doc = Document { blocks, ..Default::default() };
        doc.normalize();
        Ok((doc, blobs))
    }

    fn card(&self, title: &str, md: &str, tags: &[TagId], lock: LockMode) -> Res<cardfile_core::CardId> {
        let card = self.db.create_card(
            &self.actor,
            NewCard {
                title: title.into(),
                content: Document::default().to_json(),
                plain_text: String::new(),
                lock_mode: LockMode::None,
            },
        )?;
        self.edit(card.id, title, md, 0)?;
        self.db.set_card_tags(&self.actor, card.id, tags)?;
        if lock != LockMode::None {
            let c = self.db.card(card.id)?;
            let (doc, blobs) = self.doc(md)?;
            self.db.set_card_lock(
                &self.actor,
                card.id,
                lock,
                &c.content,
                &cardfile_doc::to_plain_text(&doc),
                None,
                &[],
                &blobs,
            )?;
        }
        Ok(card.id)
    }

    fn edit(&self, id: cardfile_core::CardId, title: &str, md: &str, keep: u32) -> Res<()> {
        let (doc, blob_ids) = self.doc(md)?;
        self.db.update_card_content(
            &self.actor,
            id,
            ContentUpdate {
                title: title.into(),
                content: doc.to_json(),
                plain_text: cardfile_doc::to_plain_text(&doc),
                encrypted: None,
                blob_ids,
            },
            keep,
            0,
        )?;
        Ok(())
    }
}

fn main() -> Res<()> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().ok_or("usage: showcase <data-dir> <images-dir>")?;
    let images = args.next().ok_or("usage: showcase <data-dir> <images-dir>")?;
    let path = std::path::Path::new(&dir).join("cardfile.db");
    if path.exists() {
        return Err(format!("{} already exists", path.display()).into());
    }
    std::fs::create_dir_all(&dir)?;
    let db = Database::open(&path, None)?;
    db.save_app_settings(&AppSettings {
        initialized: true,
        auth_mode: AuthMode::NoPassword,
        mcp_enabled: true,
        ..Default::default()
    })?;
    let user = db.create_user("alex", "Alex Rivera", None, true)?;
    db.save_user_settings(
        user.id,
        &UserSettings {
            theme: std::env::var("SHOWCASE_THEME").unwrap_or_else(|_| "Midnight".into()),
            language: std::env::var("SHOWCASE_LANG").unwrap_or_else(|_| "en".into()),
            check_for_updates: false,
            list_width: 320.0,
            ..Default::default()
        },
    )?;
    let secret = blake3::hash(b"showcase-demo-key").to_hex().to_string();
    db.create_api_key(user.id, "Claude Desktop", "cfk_7Hq2", &secret, false, false)?;
    db.create_api_key(user.id, "Cursor (read-only)", "cfk_Rm9x", &secret[..40], false, true)?;
    let s = Seed { db: &db, actor: Actor::user(user.id), images: images.into() };

    let work = db.create_tag(user.id, "Work", "#3b82f6")?.id;
    let travel = db.create_tag(user.id, "Travel", "#06b6d4")?.id;
    let recipes = db.create_tag(user.id, "Recipes", "#f97316")?.id;
    let ideas = db.create_tag(user.id, "Ideas", "#22c55e")?.id;
    let reading = db.create_tag(user.id, "Reading", "#a855f7")?.id;
    let home = db.create_tag(user.id, "Home", "#eab308")?.id;

    s.card(
        "Garden planner",
        "## Spring beds\n\n| Bed | Plant | Sow | Harvest |\n|---|---|---|---|\n| North | Lettuce | March | May |\n| South | Tomatoes | April | July |\n| East | Basil | April | June |\n| West | Strawberries | March | June |\n\n- [x] Order seeds\n- [x] Fix the drip line\n- [ ] Mulch the south bed",
        &[home],
        LockMode::None,
    )?;
    s.card(
        "Home Wi-Fi & router",
        "Network: **Rivera-Home**\n\nRouter admin is on the shelf label.",
        &[home],
        LockMode::Private,
    )?;
    s.card(
        "Git snippets",
        "Handy commands I always forget.\n\n```bash\n# Undo the last commit, keep the changes\ngit reset --soft HEAD~1\n\n# Rename the current branch\ngit branch -m new-name\n\n# Show what changed in a file\ngit log -p --follow -- path/to/file\n```\n\n> Tip: `git switch -` jumps back to the previous branch.",
        &[work, ideas],
        LockMode::None,
    )?;
    s.card(
        "Languages · 语言 · 言語 · שפות · لغات",
        "Cardfile handles every script, including right-to-left text.\n\n中文：今天的会议改到下午三点。\n\n日本語：明日は雨が降るそうです。\n\n한국어: 회의는 오후 3시입니다.\n\nעברית: הפגישה נדחתה ליום רביעי.\n\nالعربية: الاجتماع يوم الأربعاء الساعة 3.\n\nEspañol: la reunión pasó al miércoles.",
        &[ideas],
        LockMode::None,
    )?;
    s.card(
        "Reading list 2026",
        "## Currently reading\n\n- *Middlemarch* — George Eliot\n\n## Up next\n\n- [x] *The Count of Monte Cristo* — Alexandre Dumas\n- [x] *Frankenstein* — Mary Shelley\n- [ ] *Moby-Dick* — Herman Melville\n- [ ] *Don Quixote* — Miguel de Cervantes\n- [ ] *War and Peace* — Leo Tolstoy\n\n> Goal: one classic a month, notes on a card each.",
        &[reading],
        LockMode::None,
    )?;
    s.card(
        "Sourdough bread",
        "Makes one loaf. Start the night before baking.\n@img:sourdough.jpg|460\n## Ingredients\n\n| | Amount |\n|---|---|\n| Bread flour | 450 g |\n| Whole wheat flour | 50 g |\n| Water | 350 g |\n| Starter | 100 g |\n| Salt | 10 g |\n\n## Method\n\n1. Mix flour and water, rest 1 hour.\n2. Add starter and salt, then fold every 30 minutes for 2 hours.\n3. Shape, cover and proof overnight in the fridge.\n4. Bake at **250 °C** in a covered pot for 20 minutes, then 25 minutes uncovered.\n\n> Let it cool for an hour before cutting. Really.",
        &[recipes],
        LockMode::ReadOnly,
    )?;
    let trip = s.card(
        "Lisbon trip — May",
        "Three days, mostly on foot. Book the Sintra train the night before.\n@img:lisbon.jpg|720\n## Itinerary\n\n| Day | Plan |\n|---|---|\n| Fri | Alfama, tram 28, sunset at the miradouro |\n| Sat | Belém, LX Factory, dinner in Bairro Alto |\n| Sun | Day trip to Sintra |\n\n## Packing\n\n- [x] Passport\n- [x] Comfortable shoes\n- [ ] Power adapter\n- [ ] Rain jacket\n\nFlight **TP 1023** leaves at 07:40, gate closes 40 minutes before.",
        &[travel],
        LockMode::None,
    )?;
    db.set_card_favorite(&s.actor, trip, true)?;
    s.card(
        "Weekly groceries",
        "- [x] Eggs\n- [x] Oat milk\n- [ ] Tomatoes\n- [ ] Coffee beans\n- [ ] Lemons\n- [ ] Parmesan",
        &[home],
        LockMode::None,
    )?;
    s.card(
        "App ideas",
        "## Small things worth building\n\n- A *habit tracker* that only asks one question a day.\n- A recipe scaler that understands grams and cups.\n- A plant-watering reminder based on the weather forecast.\n\n---\n\nKeep ideas small enough to ship in a weekend.",
        &[ideas],
        LockMode::None,
    )?;
    s.card(
        "Design review — notes",
        "**Attendees:** Alex, Priya, Tomás, Mei\n\n## Decisions\n\n- Keep the sidebar collapsible on small screens.\n- Use the warm palette for empty states.\n- Ship dark mode with the first release.\n\n## Action items\n\n- [ ] Priya: update the icon set\n- [ ] Tomás: accessibility pass on dialogs\n- [x] Mei: share the prototype link",
        &[work],
        LockMode::None,
    )?;

    // Created last and edited a few times so it tops the list and has a version history.
    let plan = s.card(
        "Product launch plan",
        "# Launch checklist\n\n- [ ] Write the announcement",
        &[work],
        LockMode::None,
    )?;
    s.edit(
        plan,
        "Product launch plan",
        "# Launch checklist\n\n- [x] Freeze features\n- [ ] Write the announcement\n- [ ] Record the demo video",
        50,
    )?;
    s.edit(
        plan,
        "Product launch plan",
        "# Launch checklist\n\nTarget date: **November 12**. Everything below must be green a week before.\n\n- [x] Freeze features\n- [x] Final QA pass on Windows and Linux\n- [ ] Write the announcement\n- [ ] Record the demo video\n- [ ] Update the website screenshots\n\n## Owners\n\n| Area | Owner | Status |\n|---|---|---|\n| Website | Priya | In review |\n| Docs | Tomás | Done |\n| Press kit | Mei | Drafting |\n\n## Sign-ups per week\n\n@img:chart.png|560\n> Ship it when it is ready, not before.",
        50,
    )?;
    db.set_card_pinned(&s.actor, plan, true)?;
    db.set_card_favorite(&s.actor, plan, true)?;
    println!("seeded {}", path.display());
    Ok(())
}
