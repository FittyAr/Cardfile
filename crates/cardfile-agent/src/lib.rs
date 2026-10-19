//! What an AI assistant can do in Cardfile, and the rules it follows.
//!
//! The same tools back the local MCP server (external AI clients holding an API key)
//! and the in-app chat. The AI always acts through the built-in AI account on behalf
//! of one person ([`Scope`]): it sees exactly what that person sees, every change is
//! attributed to it, edits snapshot the previous version first (so they can be
//! undone from the history) and deletions go to the recycle bin.
//!
//! Encrypted cards are never available to the AI; private cards only when the scope
//! allows it.
//!
//! Every write is refused on cards locked as read-only and on cards in the recycle
//! bin (except `restore_card`). Cards in the recycle bin can still be read, so the AI
//! can tell the user what is there before restoring it.
//!
//! All functions are blocking (database access): call them off the UI thread.

pub mod content;

use std::collections::{HashMap, HashSet};

use cardfile_core::{
    Actor, Card, CardAccess, CardFilter, CardId, CardSummary, LockMode, NewCard, SearchScope,
    SortOrder, Tag, TagId, UserId, mask_title,
};
use cardfile_db::{ContentUpdate, Database};
use cardfile_doc::Document;
use serde_json::{Value, json};

/// A tool definition, independent of MCP or any LLM wire format.
#[derive(Clone, Debug)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema of the arguments.
    pub schema: Value,
    /// Only reads data.
    pub read_only: bool,
    /// May remove data (moves cards to the recycle bin).
    pub destructive: bool,
}

/// Who the AI acts for and what it may touch.
#[derive(Clone, Debug)]
pub struct Scope {
    /// The person who granted access (API key owner or chat user).
    pub principal: UserId,
    /// The built-in AI account.
    pub ai: UserId,
    /// How access was granted: the API key name, or `chat`.
    pub via: String,
    /// Allow reading and editing private cards.
    pub allow_private: bool,
    /// Only reading tools are offered and allowed.
    pub read_only: bool,
}

impl Scope {
    fn actor(&self) -> Actor {
        Actor::ai(self.ai, self.principal, self.via.clone())
    }
}

/// Result of running a tool: JSON for the AI, and whether data changed.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub value: Value,
    pub changed: bool,
    /// The card the tool created or edited, so the UI can show it.
    pub card: Option<CardId>,
}

impl Outcome {
    fn read(value: Value) -> Self {
        Self {
            value,
            changed: false,
            card: None,
        }
    }

    fn wrote(value: Value, card: Option<CardId>) -> Self {
        Self {
            value,
            changed: true,
            card,
        }
    }
}

/// Instructions for the model, shared by the MCP server and the chat.
pub const INSTRUCTIONS: &str = "You are working inside Cardfile, a personal card file (notes app). \
Each card has a title, rich content (exchanged as Markdown), tags, and flags (favorite, pinned). \
Use search_cards to find cards and get_card to read one before changing it. \
Prefer edit_card_text for small corrections because it keeps formatting such as colours; \
use update_card to rewrite or reformat a whole card. Lines like ⟦file 2: photo.png⟧ stand for \
images and attachments: keep them where they belong when rewriting. \
Card ids are UUIDs; never invent them. Deleted cards go to the recycle bin and can be restored; \
cards in the recycle bin and cards locked as read-only cannot be changed. \
Encrypted cards are never readable by you. Answer in the user's language and be concise.";

fn schema(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

/// Every tool, before scope filtering.
pub fn all_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "search_cards",
            description: "Search or list cards. Without a query, lists cards in the user's order. \
                Returns id, title, a preview, tags and flags.",
            schema: schema(
                json!({
                    "query": {"type": "string", "description": "Words to search in titles, content and tags."},
                    "tag": {"type": "string", "description": "Only cards with this tag name."},
                    "favorites_only": {"type": "boolean"},
                    "shared_with_me": {"type": "boolean", "description": "Cards other users share with the user."},
                    "in_recycle_bin": {"type": "boolean"},
                    "sort": {"type": "string", "enum": ["manual", "updated", "created", "title"]},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200}
                }),
                &[],
            ),
            read_only: true,
            destructive: false,
        },
        ToolDef {
            name: "get_card",
            description: "Read one card: title, content as Markdown, tags, flags and protection.",
            schema: schema(json!({"id": {"type": "string"}}), &["id"]),
            read_only: true,
            destructive: false,
        },
        ToolDef {
            name: "list_tags",
            description: "List the user's tags with their colours and how many cards use them.",
            schema: schema(json!({}), &[]),
            read_only: true,
            destructive: false,
        },
        ToolDef {
            name: "recent_activity",
            description: "Recent changes to the user's cards, including who made them (the user, \
                other users or the AI).",
            schema: schema(
                json!({"limit": {"type": "integer", "minimum": 1, "maximum": 100}}),
                &[],
            ),
            read_only: true,
            destructive: false,
        },
        ToolDef {
            name: "create_card",
            description: "Create a card. Content is Markdown (headings, lists, checklists `- [ ]`, \
                tables, code, quotes, links, bold/italic).",
            schema: schema(
                json!({
                    "title": {"type": "string"},
                    "markdown": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}, "description": "Tag names; missing tags are created."},
                    "favorite": {"type": "boolean"},
                    "pinned": {"type": "boolean"}
                }),
                &["title"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "update_card",
            description: "Change a card's title and/or replace its whole content with new Markdown \
                (use it to rewrite, reorganise or reformat). The previous version stays in history.",
            schema: schema(
                json!({
                    "id": {"type": "string"},
                    "title": {"type": "string"},
                    "markdown": {"type": "string"}
                }),
                &["id"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "edit_card_text",
            description: "Find and replace text inside a card, keeping its formatting. \
                Use for small corrections.",
            schema: schema(
                json!({
                    "id": {"type": "string"},
                    "find": {"type": "string"},
                    "replace": {"type": "string"},
                    "all": {"type": "boolean", "description": "Replace every occurrence (default true)."}
                }),
                &["id", "find", "replace"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "append_to_card",
            description: "Add Markdown at the end of a card.",
            schema: schema(
                json!({"id": {"type": "string"}, "markdown": {"type": "string"}}),
                &["id", "markdown"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "tag_card",
            description: "Change a card's tags by name. mode `add` (default), `remove` or `replace`. \
                `add` and `replace` create missing tags; `remove` ignores unknown ones.",
            schema: schema(
                json!({
                    "id": {"type": "string"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "mode": {"type": "string", "enum": ["add", "remove", "replace"]}
                }),
                &["id", "tags"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "create_tag",
            description: "Create a tag. Colour is #rrggbb (optional).",
            schema: schema(
                json!({"name": {"type": "string"}, "color": {"type": "string"}}),
                &["name"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "set_card_flags",
            description: "Mark a card as favorite and/or pinned (pinned cards stay at the top).",
            schema: schema(
                json!({
                    "id": {"type": "string"},
                    "favorite": {"type": "boolean"},
                    "pinned": {"type": "boolean"}
                }),
                &["id"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "reorder_cards",
            description: "Put cards in the given order in the user's manual order. The cards are \
                placed together, starting where the first of them is now.",
            schema: schema(
                json!({"ids": {"type": "array", "items": {"type": "string"}, "minItems": 2}}),
                &["ids"],
            ),
            read_only: false,
            destructive: false,
        },
        ToolDef {
            name: "delete_card",
            description: "Move a card to the recycle bin (it can be restored).",
            schema: schema(json!({"id": {"type": "string"}}), &["id"]),
            read_only: false,
            destructive: true,
        },
        ToolDef {
            name: "restore_card",
            description: "Bring a card back from the recycle bin.",
            schema: schema(json!({"id": {"type": "string"}}), &["id"]),
            read_only: false,
            destructive: false,
        },
    ]
}

/// The tools available in `scope`.
pub fn tools(scope: &Scope) -> Vec<ToolDef> {
    all_tools()
        .into_iter()
        .filter(|t| t.read_only || !scope.read_only)
        .collect()
}

/// Run a tool. Errors are messages meant for the AI.
pub fn call(db: &Database, scope: &Scope, name: &str, args: &Value) -> Result<Outcome, String> {
    let tool = all_tools()
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| format!("unknown tool `{name}`"))?;
    if !tool.read_only && scope.read_only {
        return Err("this API key is read-only".into());
    }
    let ctx = Ctx { db, scope, args };
    let result = match name {
        "search_cards" => ctx.search(),
        "get_card" => ctx.get(),
        "list_tags" => ctx.list_tags(),
        "recent_activity" => ctx.activity(),
        "create_card" => ctx.create(),
        "update_card" => ctx.update(),
        "edit_card_text" => ctx.edit_text(),
        "append_to_card" => ctx.append(),
        "tag_card" => ctx.tag(),
        "create_tag" => ctx.create_tag(),
        "set_card_flags" => ctx.flags(),
        "reorder_cards" => ctx.reorder(),
        "delete_card" => ctx.delete(),
        "restore_card" => ctx.restore(),
        _ => Err(format!("unknown tool `{name}`")),
    };
    if let Err(e) = &result {
        tracing::debug!(tool = name, "AI tool failed: {e}");
    }
    result
}

struct Ctx<'a> {
    db: &'a Database,
    scope: &'a Scope,
    args: &'a Value,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Ctx<'_> {
    fn str(&self, key: &str) -> Option<&str> {
        self.args.get(key).and_then(Value::as_str)
    }

    fn bool(&self, key: &str) -> Option<bool> {
        self.args.get(key).and_then(Value::as_bool)
    }

    fn strings(&self, key: &str) -> Vec<String> {
        self.args
            .get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn id(&self) -> Result<CardId, String> {
        let raw = self.str("id").ok_or("missing `id`")?;
        raw.trim()
            .parse()
            .map_err(|_| format!("`{raw}` is not a card id"))
    }

    fn tags(&self) -> Result<Vec<Tag>, String> {
        self.db.list_tags(self.scope.principal).map_err(err)
    }

    fn tag_names(&self, ids: &[TagId], tags: &[Tag]) -> Vec<String> {
        tags.iter()
            .filter(|t| ids.contains(&t.id))
            .map(|t| t.name.clone())
            .collect()
    }

    /// Load a card the scope may see. With `write`, also check edit rights.
    fn card(&self, id: CardId, write: bool) -> Result<(Card, CardAccess), String> {
        let access = self
            .db
            .card_access(id, self.scope.principal)
            .map_err(err)?
            .ok_or("card not found")?;
        let card = self.db.card(id).map_err(err)?;
        match card.lock_mode {
            LockMode::Encrypted => {
                return Err("this card is encrypted; the AI cannot open it".into());
            }
            LockMode::Private if !self.scope.allow_private => {
                return Err(
                    "this card is private and this AI access is not allowed to open private cards"
                        .into(),
                );
            }
            _ => {}
        }
        if write {
            if !access.can_edit() {
                return Err("the card is shared with the user as view-only".into());
            }
            writable(&card)?;
        }
        Ok((card, access))
    }

    /// The title as the scope may see it: masked for private cards the scope
    /// cannot open (the same rule as the app's card list).
    fn visible_title(&self, title: &str, lock_mode: LockMode) -> String {
        if lock_mode == LockMode::Private && !self.scope.allow_private {
            mask_title(title, MASK_VISIBLE)
        } else {
            title.to_owned()
        }
    }

    fn summary_json(&self, c: &CardSummary, tags: &[Tag]) -> Value {
        let hidden = c.lock_mode == LockMode::Encrypted
            || (c.lock_mode == LockMode::Private && !self.scope.allow_private);
        let title = self.visible_title(&c.title, c.lock_mode);
        let mut v = json!({
            "id": c.id.to_string(),
            "title": title,
            "tags": self.tag_names(&c.tags, tags),
            "favorite": c.is_favorite,
            "pinned": c.is_pinned,
            "protection": c.lock_mode.as_str(),
            "updated_at": c.updated_at,
        });
        if !hidden {
            v["preview"] = json!(c.preview);
        }
        if let Some(s) = c.snippet.as_ref().filter(|_| !hidden) {
            v["match"] = json!(s.replace(['\u{1}', '\u{2}'], "**"));
        }
        if !c.access.is_owner() {
            v["shared_with_me"] = json!(true);
            v["can_edit"] = json!(c.access.can_edit());
        }
        v
    }

    fn search(&self) -> Result<Outcome, String> {
        let tags = self.tags()?;
        let tag = match self.str("tag") {
            Some(name) => Some(
                tags.iter()
                    .find(|t| t.name.eq_ignore_ascii_case(name.trim()))
                    .map(|t| t.id)
                    .ok_or_else(|| format!("no tag named `{name}`"))?,
            ),
            None => None,
        };
        let sort = match self.str("sort") {
            Some("updated") => SortOrder::UpdatedDesc,
            Some("created") => SortOrder::CreatedDesc,
            Some("title") => SortOrder::TitleAsc,
            _ => SortOrder::Manual,
        };
        let filter = CardFilter {
            query: self.str("query").unwrap_or_default().to_owned(),
            scope: SearchScope::Everything,
            tag,
            favorites_only: self.bool("favorites_only").unwrap_or(false),
            deleted: self.bool("in_recycle_bin").unwrap_or(false),
            shared_with_me: self.bool("shared_with_me").unwrap_or(false),
            sort,
        };
        let limit = self
            .args
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(50)
            .clamp(1, 200) as usize;
        let cards = self
            .db
            .list_cards(self.scope.principal, &filter)
            .map_err(err)?;
        let total = cards.len();
        let items: Vec<Value> = cards
            .iter()
            .take(limit)
            .map(|c| self.summary_json(c, &tags))
            .collect();
        Ok(Outcome::read(json!({ "total": total, "cards": items })))
    }

    fn get(&self) -> Result<Outcome, String> {
        let (card, access) = self.card(self.id()?, false)?;
        let doc = Document::from_json(&card.content).map_err(err)?;
        let tags = self.tags()?;
        let updated_by = card
            .updated_by
            .and_then(|u| self.db.user(u).ok())
            .map(|u| u.display_name);
        Ok(Outcome::read(json!({
            "id": card.id.to_string(),
            "title": card.title,
            "markdown": content::to_markdown(&doc),
            "tags": self.tag_names(&card.tags, &tags),
            "favorite": card.is_favorite,
            "pinned": card.is_pinned,
            "protection": card.lock_mode.as_str(),
            "in_recycle_bin": card.is_deleted(),
            "can_edit": access.can_edit() && card.lock_mode != LockMode::ReadOnly,
            "updated_at": card.updated_at,
            "updated_by": updated_by,
        })))
    }

    fn list_tags(&self) -> Result<Outcome, String> {
        let tags = self.tags()?;
        let cards = self
            .db
            .list_cards(self.scope.principal, &CardFilter::default())
            .map_err(err)?;
        let mut counts: HashMap<TagId, usize> = HashMap::new();
        for c in &cards {
            for t in &c.tags {
                *counts.entry(*t).or_default() += 1;
            }
        }
        let items: Vec<Value> = tags
            .iter()
            .map(|t| json!({"name": t.name, "color": t.color, "cards": counts.get(&t.id).copied().unwrap_or(0)}))
            .collect();
        Ok(Outcome::read(json!({ "tags": items })))
    }

    fn activity(&self) -> Result<Outcome, String> {
        let limit = self
            .args
            .get("limit")
            .and_then(Value::as_i64)
            .unwrap_or(20)
            .clamp(1, 100);
        let entries = self
            .db
            .activity(None, Some(self.scope.principal), limit)
            .map_err(err)?;
        let mut names: HashMap<UserId, String> = HashMap::new();
        let mut name = |id: UserId| {
            names
                .entry(id)
                .or_insert_with(|| self.db.user(id).map(|u| u.display_name).unwrap_or_default())
                .clone()
        };
        // The summary is the card title at the time of the change: mask it like the
        // card list does, using the card's current protection. Cards the user can no
        // longer see (unshared or purged) are masked too, since their protection is
        // unknown.
        let mut locks: HashMap<CardId, Option<LockMode>> = HashMap::new();
        let mut lock_of = |id: CardId| {
            *locks.entry(id).or_insert_with(|| {
                match self.db.card_access(id, self.scope.principal) {
                    Ok(Some(_)) => self.db.card(id).ok().map(|c| c.lock_mode),
                    _ => None,
                }
            })
        };
        let items: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                let title = match e.card_id {
                    None => e.summary.clone(),
                    Some(id) => match lock_of(id) {
                        Some(mode) => self.visible_title(&e.summary, mode),
                        None => mask_title(&e.summary, MASK_VISIBLE),
                    },
                };
                json!({
                    "action": e.action,
                    "card_title": title,
                    "card_id": e.card_id.map(|c| c.to_string()),
                    "by": name(e.actor_id),
                    "via": e.via,
                    "at": e.created_at,
                })
            })
            .collect();
        Ok(Outcome::read(json!({ "activity": items })))
    }

    /// Find existing tags by name; unknown names are ignored.
    fn existing_tags(&self, names: &[String]) -> Result<Vec<TagId>, String> {
        let tags = self.tags()?;
        Ok(names
            .iter()
            .filter_map(|n| tags.iter().find(|t| t.name.eq_ignore_ascii_case(n)))
            .map(|t| t.id)
            .collect())
    }

    /// Find tags by name, creating missing ones.
    fn resolve_tags(&self, names: &[String]) -> Result<Vec<TagId>, String> {
        let mut tags = self.tags()?;
        let mut ids = Vec::new();
        for name in names {
            let id = match tags.iter().find(|t| t.name.eq_ignore_ascii_case(name)) {
                Some(t) => t.id,
                None => {
                    let color = PALETTE[tags.len() % PALETTE.len()];
                    let tag = self
                        .db
                        .create_tag(self.scope.principal, name, color)
                        .map_err(err)?;
                    let id = tag.id;
                    tags.push(tag);
                    id
                }
            };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    fn save(&self, card: &Card, title: String, doc: &Document) -> Result<(), String> {
        let keep = self
            .db
            .user_settings(card.owner_id)
            .map(|s| s.max_revisions)
            .unwrap_or(50);
        let update = ContentUpdate {
            title,
            content: doc.to_json(),
            plain_text: cardfile_doc::to_plain_text(doc),
            encrypted: None,
            blob_ids: doc.blob_ids(),
        };
        // Snapshot before every AI edit, so the user can always go back.
        self.db
            .update_card_content(&self.scope.actor(), card.id, update, keep.max(1), 0)
            .map_err(err)?;
        Ok(())
    }

    fn create(&self) -> Result<Outcome, String> {
        let title = self.str("title").unwrap_or_default().trim().to_owned();
        let doc = content::from_markdown(self.str("markdown").unwrap_or_default(), None);
        let actor = self.scope.actor();
        let card = self
            .db
            .create_card(
                &actor,
                NewCard {
                    title: title.clone(),
                    content: doc.to_json(),
                    plain_text: cardfile_doc::to_plain_text(&doc),
                    lock_mode: LockMode::None,
                },
            )
            .map_err(err)?;
        let names = self.strings("tags");
        if !names.is_empty() {
            let ids = self.resolve_tags(&names)?;
            self.db.set_card_tags(&actor, card.id, &ids).map_err(err)?;
        }
        if self.bool("favorite") == Some(true) {
            self.db
                .set_card_favorite(&actor, card.id, true)
                .map_err(err)?;
        }
        if self.bool("pinned") == Some(true) {
            self.db
                .set_card_pinned(&actor, card.id, true)
                .map_err(err)?;
        }
        Ok(Outcome::wrote(
            json!({"id": card.id.to_string(), "title": title}),
            Some(card.id),
        ))
    }

    fn update(&self) -> Result<Outcome, String> {
        let (card, _) = self.card(self.id()?, true)?;
        let previous = Document::from_json(&card.content).map_err(err)?;
        let title = self
            .str("title")
            .map(|t| t.trim().to_owned())
            .unwrap_or_else(|| card.title.clone());
        let doc = match self.str("markdown") {
            Some(md) => content::from_markdown(md, Some(&previous)),
            None => previous,
        };
        self.save(&card, title, &doc)?;
        Ok(Outcome::wrote(json!({"ok": true}), Some(card.id)))
    }

    fn edit_text(&self) -> Result<Outcome, String> {
        let (card, _) = self.card(self.id()?, true)?;
        let find = self.str("find").ok_or("missing `find`")?;
        if find.is_empty() {
            return Err("`find` must not be empty".into());
        }
        let replace = self.str("replace").ok_or("missing `replace`")?;
        let all = self.bool("all").unwrap_or(true);
        let mut doc = Document::from_json(&card.content).map_err(err)?;
        let mut title = card.title.clone();
        let mut n = content::replace_text(&mut doc, find, replace, all);
        if (all || n == 0) && title.contains(find) {
            title = if all {
                title.replace(find, replace)
            } else {
                title.replacen(find, replace, 1)
            };
            n += 1;
        }
        if n == 0 {
            return Err(format!("`{find}` was not found in the card"));
        }
        self.save(&card, title, &doc)?;
        Ok(Outcome::wrote(json!({"replaced": n}), Some(card.id)))
    }

    fn append(&self) -> Result<Outcome, String> {
        let (card, _) = self.card(self.id()?, true)?;
        let mut doc = Document::from_json(&card.content).map_err(err)?;
        let extra = content::from_markdown(self.str("markdown").unwrap_or_default(), None);
        // A new card holds one empty paragraph; replace it rather than leave a gap.
        if doc.blocks.len() == 1 && doc.blocks[0].text.is_empty() && doc.blocks[0].kind.is_textual()
        {
            doc.blocks.clear();
        }
        doc.blocks.extend(extra.blocks);
        doc.normalize();
        self.save(&card, card.title.clone(), &doc)?;
        Ok(Outcome::wrote(json!({"ok": true}), Some(card.id)))
    }

    /// A card the user owns, for metadata changes. Refused on read-only cards and
    /// cards in the recycle bin.
    fn owned_card(&self) -> Result<Card, String> {
        let card = self.owned_card_any(self.id()?)?;
        writable(&card)?;
        Ok(card)
    }

    /// A card the user owns, whatever its state (for `restore_card`).
    fn owned_card_any(&self, id: CardId) -> Result<Card, String> {
        let (card, access) = self.card_meta(id)?;
        if !access.is_owner() {
            return Err("only the card's owner can do that".into());
        }
        Ok(card)
    }

    /// Load a card for metadata changes (tags, flags, order, recycle bin), which do
    /// not reveal content and so work on private and encrypted cards too. Callers
    /// that write must also check [`writable`].
    fn card_meta(&self, id: CardId) -> Result<(Card, CardAccess), String> {
        let access = self
            .db
            .card_access(id, self.scope.principal)
            .map_err(err)?
            .ok_or("card not found")?;
        Ok((self.db.card(id).map_err(err)?, access))
    }

    fn tag(&self) -> Result<Outcome, String> {
        let card = self.owned_card()?;
        let names = self.strings("tags");
        let mut tags = card.tags.clone();
        match self.str("mode").unwrap_or("add") {
            "replace" => tags = self.resolve_tags(&names)?,
            // Removing never creates tags: unknown names are simply not on the card.
            "remove" => {
                let ids = self.existing_tags(&names)?;
                tags.retain(|t| !ids.contains(t));
            }
            _ => {
                for id in self.resolve_tags(&names)? {
                    if !tags.contains(&id) {
                        tags.push(id);
                    }
                }
            }
        }
        self.db
            .set_card_tags(&self.scope.actor(), card.id, &tags)
            .map_err(err)?;
        let all = self.tags()?;
        Ok(Outcome::wrote(
            json!({"tags": self.tag_names(&tags, &all)}),
            Some(card.id),
        ))
    }

    fn create_tag(&self) -> Result<Outcome, String> {
        let name = self.str("name").unwrap_or_default().trim();
        if name.is_empty() {
            return Err("missing `name`".into());
        }
        let tags = self.tags()?;
        if tags.iter().any(|t| t.name.eq_ignore_ascii_case(name)) {
            return Err(format!("a tag named `{name}` already exists"));
        }
        let color = self
            .str("color")
            .filter(|c| c.len() == 7 && c.starts_with('#'))
            .unwrap_or(PALETTE[tags.len() % PALETTE.len()]);
        let tag = self
            .db
            .create_tag(self.scope.principal, name, color)
            .map_err(err)?;
        Ok(Outcome::wrote(
            json!({"name": tag.name, "color": tag.color}),
            None,
        ))
    }

    fn flags(&self) -> Result<Outcome, String> {
        let card = self.owned_card()?;
        let actor = self.scope.actor();
        if let Some(f) = self.bool("favorite") {
            self.db.set_card_favorite(&actor, card.id, f).map_err(err)?;
        }
        if let Some(p) = self.bool("pinned") {
            self.db.set_card_pinned(&actor, card.id, p).map_err(err)?;
        }
        Ok(Outcome::wrote(json!({"ok": true}), Some(card.id)))
    }

    fn reorder(&self) -> Result<Outcome, String> {
        let raw = self.strings("ids");
        if raw.len() > MAX_REORDER {
            return Err(format!("give at most {MAX_REORDER} card ids at a time"));
        }
        let ids: Vec<CardId> = raw
            .iter()
            .map(|s| s.parse().map_err(|_| format!("`{s}` is not a card id")))
            .collect::<Result<_, _>>()?;
        if ids.len() < 2 {
            return Err("give at least two card ids".into());
        }
        let mut seen = HashSet::new();
        if let Some(dup) = ids.iter().find(|id| !seen.insert(**id)) {
            return Err(format!("card `{dup}` is listed more than once"));
        }
        // The live manual order, loaded once and then kept in step with each move.
        let mut order: Vec<CardId> = self
            .db
            .list_cards(self.scope.principal, &CardFilter::default())
            .map_err(err)?
            .into_iter()
            .map(|c| c.id)
            .collect();
        let live: HashSet<CardId> = order.iter().copied().collect();
        for id in &ids {
            if !live.contains(id) {
                return Err(format!(
                    "card `{id}` is not in the user's card list (it may be in the recycle bin)"
                ));
            }
            let (card, access) = self.card_meta(*id)?;
            if !access.is_owner() {
                return Err("only the user's own cards can be reordered".into());
            }
            writable(&card)?;
        }
        let actor = self.scope.actor();
        for pair in ids.windows(2) {
            let (prev, id) = (pair[0], pair[1]);
            // Place `id` right after `prev`, before whatever follows `prev` now.
            order.retain(|c| *c != id);
            let at = order
                .iter()
                .position(|c| *c == prev)
                .ok_or("the card list changed while reordering")?
                + 1;
            let next = order.get(at).copied();
            self.db
                .move_card(&actor, id, Some(prev), next)
                .map_err(err)?;
            order.insert(at, id);
        }
        Ok(Outcome::wrote(json!({"ok": true}), None))
    }

    fn delete(&self) -> Result<Outcome, String> {
        let card = self.owned_card()?;
        self.db
            .delete_card(&self.scope.actor(), card.id)
            .map_err(err)?;
        Ok(Outcome::wrote(
            json!({"ok": true, "note": "moved to the recycle bin"}),
            None,
        ))
    }

    fn restore(&self) -> Result<Outcome, String> {
        let card = self.owned_card_any(self.id()?)?;
        if !card.is_deleted() {
            return Err("the card is not in the recycle bin".into());
        }
        self.db
            .restore_card(&self.scope.actor(), card.id)
            .map_err(err)?;
        Ok(Outcome::wrote(json!({"ok": true}), Some(card.id)))
    }
}

/// Characters left visible when a private card's title is masked (the app's default).
const MASK_VISIBLE: usize = 3;

/// Most cards `reorder_cards` accepts in one call.
const MAX_REORDER: usize = 200;

/// Refuse changes to cards the user protected or threw away.
fn writable(card: &Card) -> Result<(), String> {
    if card.is_deleted() {
        return Err("the card is in the recycle bin; restore it before changing it".into());
    }
    if card.lock_mode == LockMode::ReadOnly {
        return Err("the card is locked as read-only; only the user can unlock it".into());
    }
    Ok(())
}

/// Colours for tags the AI creates.
const PALETTE: &[&str] = &[
    "#3b82f6", "#22c55e", "#f97316", "#a855f7", "#ef4444", "#14b8a6", "#eab308", "#ec4899",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(read_only: bool, allow_private: bool) -> (Database, Scope) {
        let db = Database::open_in_memory().unwrap();
        let user = db.create_user("ana", "Ana", None, true).unwrap();
        let ai = db.ai_user().unwrap();
        let scope = Scope {
            principal: user.id,
            ai: ai.id,
            via: "test".into(),
            allow_private,
            read_only,
        };
        (db, scope)
    }

    fn run(db: &Database, scope: &Scope, name: &str, args: Value) -> Result<Value, String> {
        call(db, scope, name, &args).map(|o| o.value)
    }

    #[test]
    fn create_search_update_and_attribute() {
        let (db, scope) = setup(false, false);
        let created = run(
            &db,
            &scope,
            "create_card",
            json!({"title": "Groceries", "markdown": "- milk\n- eggs", "tags": ["Home"]}),
        )
        .unwrap();
        let id = created["id"].as_str().unwrap().to_owned();
        let found = run(&db, &scope, "search_cards", json!({"query": "milk"})).unwrap();
        assert_eq!(found["total"], 1);
        assert_eq!(found["cards"][0]["tags"][0], "Home");
        run(
            &db,
            &scope,
            "edit_card_text",
            json!({"id": id, "find": "eggs", "replace": "bread"}),
        )
        .unwrap();
        let card = run(&db, &scope, "get_card", json!({"id": id})).unwrap();
        assert!(card["markdown"].as_str().unwrap().contains("bread"));
        assert_eq!(card["updated_by"], "Cardfile AI");
        // Every AI edit left a revision behind.
        let cid: CardId = id.parse().unwrap();
        assert!(!db.card_revisions(cid).unwrap().is_empty());
        // Activity records the AI acting for Ana.
        let log = db.activity(Some(cid), None, 10).unwrap();
        assert!(log.iter().all(|e| e.actor_id == scope.ai));
        assert!(log.iter().all(|e| e.on_behalf_of == Some(scope.principal)));
        run(&db, &scope, "delete_card", json!({"id": id})).unwrap();
        let bin = run(&db, &scope, "search_cards", json!({"in_recycle_bin": true})).unwrap();
        assert_eq!(bin["total"], 1);
    }

    #[test]
    fn protected_cards_and_read_only_keys() {
        let (db, scope) = setup(false, false);
        let actor = Actor::user(scope.principal);
        let private = db
            .create_card(
                &actor,
                NewCard {
                    title: "Diary entry".into(),
                    content: Document::default().to_json(),
                    lock_mode: LockMode::Private,
                    ..Default::default()
                },
            )
            .unwrap();
        let id = json!({"id": private.id.to_string()});
        assert!(run(&db, &scope, "get_card", id.clone()).is_err());
        let list = run(&db, &scope, "search_cards", json!({})).unwrap();
        assert_eq!(list["cards"][0]["title"], "Dia•• •••••");
        assert!(list["cards"][0].get("preview").is_none());
        let allowed = Scope {
            allow_private: true,
            ..scope.clone()
        };
        assert!(run(&db, &allowed, "get_card", id.clone()).is_ok());
        let ro = Scope {
            read_only: true,
            ..scope.clone()
        };
        assert!(tools(&ro).iter().all(|t| t.read_only));
        assert!(run(&db, &ro, "create_card", json!({"title": "x"})).is_err());
    }

    #[test]
    fn reorder_follows_the_given_order() {
        let (db, scope) = setup(false, false);
        let mut ids = Vec::new();
        for t in ["a", "b", "c"] {
            let v = run(&db, &scope, "create_card", json!({"title": t})).unwrap();
            ids.push(v["id"].as_str().unwrap().to_owned());
        }
        // Current order: c, b, a. Ask for a, b, c.
        run(&db, &scope, "reorder_cards", json!({"ids": ids})).unwrap();
        let list = run(&db, &scope, "search_cards", json!({})).unwrap();
        let titles: Vec<&str> = list["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, ["a", "b", "c"]);
    }

    fn user_card(db: &Database, scope: &Scope, title: &str, lock_mode: LockMode) -> String {
        db.create_card(
            &Actor::user(scope.principal),
            NewCard {
                title: title.into(),
                content: Document::default().to_json(),
                lock_mode,
                ..Default::default()
            },
        )
        .unwrap()
        .id
        .to_string()
    }

    fn write_calls(id: &str) -> Vec<(&'static str, Value)> {
        vec![
            ("update_card", json!({"id": id, "title": "x"})),
            (
                "edit_card_text",
                json!({"id": id, "find": "Notes", "replace": "x"}),
            ),
            ("append_to_card", json!({"id": id, "markdown": "x"})),
            ("tag_card", json!({"id": id, "tags": ["t"]})),
            ("set_card_flags", json!({"id": id, "favorite": true})),
            ("delete_card", json!({"id": id})),
        ]
    }

    #[test]
    fn read_only_cards_refuse_every_write() {
        let (db, scope) = setup(false, true);
        let id = user_card(&db, &scope, "Notes", LockMode::ReadOnly);
        let other = user_card(&db, &scope, "Other", LockMode::None);
        for (tool, args) in write_calls(&id) {
            let e = run(&db, &scope, tool, args).unwrap_err();
            assert!(e.contains("read-only"), "{tool}: {e}");
        }
        let e = run(&db, &scope, "reorder_cards", json!({"ids": [other, id]})).unwrap_err();
        assert!(e.contains("read-only"), "{e}");
        let card = db.card(id.parse().unwrap()).unwrap();
        assert_eq!(card.title, "Notes");
        assert!(!card.is_deleted() && !card.is_favorite && card.tags.is_empty());
        assert!(run(&db, &scope, "get_card", json!({"id": id})).is_ok());
    }

    #[test]
    fn recycled_cards_refuse_writes_but_can_be_read_and_restored() {
        let (db, scope) = setup(false, false);
        let id = user_card(&db, &scope, "Notes", LockMode::None);
        run(&db, &scope, "delete_card", json!({"id": id})).unwrap();
        for (tool, args) in write_calls(&id) {
            let e = run(&db, &scope, tool, args).unwrap_err();
            assert!(e.contains("recycle bin"), "{tool}: {e}");
        }
        let card = run(&db, &scope, "get_card", json!({"id": id})).unwrap();
        assert_eq!(card["in_recycle_bin"], true);
        run(&db, &scope, "restore_card", json!({"id": id})).unwrap();
        assert!(run(&db, &scope, "restore_card", json!({"id": id})).is_err());
        run(
            &db,
            &scope,
            "update_card",
            json!({"id": id, "title": "New"}),
        )
        .unwrap();
    }

    #[test]
    fn edit_text_rejects_empty_find() {
        let (db, scope) = setup(false, false);
        let id = user_card(&db, &scope, "Title", LockMode::None);
        let e = run(
            &db,
            &scope,
            "edit_card_text",
            json!({"id": id, "find": "", "replace": "x"}),
        )
        .unwrap_err();
        assert!(e.contains("empty"), "{e}");
        assert_eq!(db.card(id.parse().unwrap()).unwrap().title, "Title");
    }

    #[test]
    fn removing_tags_never_creates_them() {
        let (db, scope) = setup(false, false);
        let id = run(
            &db,
            &scope,
            "create_card",
            json!({"title": "a", "tags": ["Keep", "Drop"]}),
        )
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let v = run(
            &db,
            &scope,
            "tag_card",
            json!({"id": id, "tags": ["drop", "Ghost"], "mode": "remove"}),
        )
        .unwrap();
        assert_eq!(v["tags"], json!(["Keep"]));
        let names: Vec<String> = db
            .list_tags(scope.principal)
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert!(!names.iter().any(|n| n == "Ghost"), "{names:?}");
    }

    #[test]
    fn activity_masks_private_titles() {
        let (db, scope) = setup(false, false);
        let id = user_card(&db, &scope, "Open", LockMode::None);
        // An AI edit on behalf of the user, then the user makes the card private.
        run(
            &db,
            &scope,
            "update_card",
            json!({"id": id, "title": "Diary entry"}),
        )
        .unwrap();
        let cid: CardId = id.parse().unwrap();
        let card = db.card(cid).unwrap();
        db.set_card_lock(
            &Actor::user(scope.principal),
            cid,
            LockMode::Private,
            &card.content,
            "",
            None,
            &[],
            &[],
        )
        .unwrap();
        let log = run(&db, &scope, "recent_activity", json!({})).unwrap();
        let titles: Vec<&str> = log["activity"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["card_title"].as_str())
            .collect();
        assert!(!titles.is_empty());
        assert!(titles.iter().all(|t| !t.contains("entry")), "{titles:?}");
        let allowed = Scope {
            allow_private: true,
            ..scope.clone()
        };
        let log = run(&db, &allowed, "recent_activity", json!({})).unwrap();
        assert!(log.to_string().contains("Diary entry"));
    }

    #[test]
    fn reorder_validates_ids() {
        let (db, scope) = setup(false, false);
        let ids: Vec<String> = ["a", "b", "c"]
            .iter()
            .map(|t| user_card(&db, &scope, t, LockMode::None))
            .collect();
        let dup = json!({"ids": [ids[0], ids[1], ids[0]]});
        assert!(
            run(&db, &scope, "reorder_cards", dup)
                .unwrap_err()
                .contains("more than once")
        );
        run(&db, &scope, "delete_card", json!({"id": ids[2]})).unwrap();
        let recycled = json!({"ids": [ids[0], ids[2]]});
        assert!(
            run(&db, &scope, "reorder_cards", recycled)
                .unwrap_err()
                .contains("recycle bin")
        );
        let many: Vec<String> = (0..=MAX_REORDER).map(|_| ids[0].clone()).collect();
        assert!(
            run(&db, &scope, "reorder_cards", json!({"ids": many}))
                .unwrap_err()
                .contains("at most")
        );
    }

    #[test]
    fn reorder_places_cards_together_after_the_first() {
        let (db, scope) = setup(false, false);
        let mut ids = Vec::new();
        for t in ["a", "b", "c", "d", "e"] {
            ids.push(user_card(&db, &scope, t, LockMode::None));
        }
        // Current order: e, d, c, b, a. Ask for d, a, c.
        let ask = json!({"ids": [ids[3], ids[0], ids[2]]});
        run(&db, &scope, "reorder_cards", ask).unwrap();
        let list = run(&db, &scope, "search_cards", json!({})).unwrap();
        let titles: Vec<&str> = list["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["title"].as_str().unwrap())
            .collect();
        assert_eq!(titles, ["e", "d", "a", "c", "b"]);
    }
}
