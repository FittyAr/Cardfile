//! The local MCP server: lets AI clients use Cardfile with an API key while the app
//! is open.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cardfile_core::ApiKeyId;
use cardfile_db::Database;
use cardfile_mcp::{Handler, ServerConfig, ServerHandle, ServerInfo, Tool, ToolOutput};
use gpui_kit::{App, AppContext as _, Global};

/// Bumped whenever an AI client changes data, so open views can refresh.
pub static DATA_VERSION: AtomicU64 = AtomicU64::new(0);

/// Record that data changed outside the UI.
pub fn bump_data_version() {
    DATA_VERSION.fetch_add(1, Ordering::Relaxed);
}

/// Who is calling: the API key and the scope it grants.
#[derive(Clone, Debug)]
pub struct Principal {
    key: ApiKeyId,
    scope: cardfile_agent::Scope,
}

impl PartialEq for Principal {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

struct CardfileHandler {
    db: Database,
}

impl Handler for CardfileHandler {
    type Principal = Principal;

    fn authenticate(&self, token: &str) -> Option<Principal> {
        let hash = cardfile_crypto::hash_api_key(token);
        let key = self
            .db
            .use_api_key(&hash)
            .map_err(|e| tracing::warn!("api key lookup: {e}"))
            .ok()??;
        // The person who created the key must still exist.
        let user = self.db.user(key.user_id).ok()?;
        if user.kind != cardfile_core::UserKind::Human {
            return None;
        }
        let ai = self.db.ai_user().ok()?;
        Some(Principal {
            key: key.id,
            scope: cardfile_agent::Scope {
                principal: key.user_id,
                ai: ai.id,
                via: key.name,
                allow_private: key.allow_private,
                read_only: key.read_only,
            },
        })
    }

    fn tools(&self, who: &Principal) -> Vec<Tool> {
        cardfile_agent::tools(&who.scope)
            .into_iter()
            .map(|t| {
                let tool = Tool::new(t.name, t.description, t.schema);
                if t.read_only {
                    tool
                } else {
                    tool.writes(t.destructive)
                }
            })
            .collect()
    }

    fn call_tool(
        &self,
        who: &Principal,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolOutput, String> {
        let outcome = cardfile_agent::call(&self.db, &who.scope, name, &arguments)?;
        if outcome.changed {
            bump_data_version();
        }
        Ok(ToolOutput::Json(outcome.value))
    }
}

/// Start the MCP server for `db` on 127.0.0.1:`port`.
fn start_server(db: Database, port: u16) -> Result<ServerHandle, cardfile_mcp::McpError> {
    let info = ServerInfo::new("cardfile", env!("CARGO_PKG_VERSION"))
        .with_title("Cardfile")
        .with_instructions(cardfile_agent::INSTRUCTIONS);
    cardfile_mcp::start(
        ServerConfig::with_port(port),
        info,
        Arc::new(CardfileHandler { db }),
    )
}

/// State of the server, shown in Settings.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Stopped,
    Running(String),
    Failed(String),
}

/// The running server, shared with the background task that starts and stops it.
#[derive(Default)]
struct Worker {
    handle: Option<ServerHandle>,
}

#[derive(Default)]
pub struct McpServer {
    /// Locked only on the background executor: stopping joins the server threads.
    worker: Arc<parking_lot::Mutex<Worker>>,
    /// Locked briefly from the UI (render) and the background task.
    status: Arc<parking_lot::Mutex<Status>>,
    /// Generation of the latest [`Self::sync`] request; older requests are skipped.
    generation: Arc<AtomicU64>,
}

impl Global for McpServer {}

impl McpServer {
    pub fn status(cx: &App) -> Status {
        cx.try_global::<Self>()
            .map(|s| s.status.lock().clone())
            .unwrap_or_default()
    }

    /// Start or stop the server to match the installation settings. The work
    /// (binding, joining the old server's threads) runs on the background executor.
    pub fn sync(cx: &mut App) {
        let state = crate::state::AppState::get(cx);
        let (enabled, port, db) = (state.app.mcp_enabled, state.app.mcp_port, state.db.clone());
        if !cx.has_global::<Self>() {
            cx.set_global(Self::default());
        }
        let server = cx.global::<Self>();
        let (worker, status, generation) = (
            server.worker.clone(),
            server.status.clone(),
            server.generation.clone(),
        );
        let ticket = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let task = cx.background_spawn(async move {
            let mut worker = worker.lock();
            // A newer request is waiting for the lock and applies its own settings.
            if generation.load(Ordering::SeqCst) != ticket {
                return;
            }
            let current_port = worker.handle.as_ref().map(|h| h.local_addr().port());
            if enabled && current_port == Some(port) {
                return;
            }
            if let Some(handle) = worker.handle.take() {
                handle.stop();
            }
            *status.lock() = Status::Stopped;
            let (true, Some(db)) = (enabled, db) else {
                return;
            };
            let new_status = match start_server(db, port) {
                Ok(handle) => {
                    tracing::info!(url = %handle.url(), "MCP server started");
                    let url = handle.url();
                    worker.handle = Some(handle);
                    Status::Running(url)
                }
                Err(e) => {
                    tracing::error!("MCP server: {e}");
                    Status::Failed(e.to_string())
                }
            };
            *status.lock() = new_status;
        });
        // Show the new status (e.g. in Settings) once the server is up or failed.
        cx.spawn(async move |cx| {
            task.await;
            cx.update(|cx| cx.refresh_windows());
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn post(url: &str, headers: &[(&str, &str)], body: Value) -> (u16, Option<String>, Value) {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        let mut req = agent
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let mut resp = req.send(body.to_string()).unwrap();
        let session = resp
            .headers()
            .get("mcp-session-id")
            .map(|v| v.to_str().unwrap().to_owned());
        let status = resp.status().as_u16();
        let text = resp.body_mut().read_to_string().unwrap();
        (
            status,
            session,
            serde_json::from_str(&text).unwrap_or(Value::Null),
        )
    }

    #[test]
    fn ai_client_uses_cardfile_with_an_api_key() {
        let db = Database::open_in_memory().unwrap();
        let ana = db.create_user("ana", "Ana", None, true).unwrap();
        let (secret, prefix, hash) = cardfile_crypto::new_api_key().unwrap();
        db.create_api_key(ana.id, "Claude", &prefix, &hash, false, false)
            .unwrap();
        let server = start_server(db.clone(), 0).unwrap();
        let url = server.url();
        let auth = format!("Bearer {}", *secret);

        // A wrong key is rejected.
        let (status, _, _) = post(
            &url,
            &[("Authorization", "Bearer nope")],
            json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}),
        );
        assert_eq!(status, 401);

        let version = "2025-06-18";
        let (status, session, _) = post(
            &url,
            &[("Authorization", &auth)],
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion": version, "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"}}}),
        );
        assert_eq!(status, 200);
        let session = session.unwrap();
        let headers = [
            ("Authorization", auth.as_str()),
            ("Mcp-Session-Id", session.as_str()),
            ("MCP-Protocol-Version", version),
        ];
        let (_, _, list) = post(
            &url,
            &headers,
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        );
        let names: Vec<&str> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"create_card") && names.contains(&"search_cards"));

        let before = DATA_VERSION.load(Ordering::Relaxed);
        let (_, _, created) = post(
            &url,
            &headers,
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
                "name": "create_card",
                "arguments": {"title": "From Claude", "markdown": "- one
- two"}}}),
        );
        assert_eq!(created["result"]["isError"], json!(false), "{created}");
        assert!(DATA_VERSION.load(Ordering::Relaxed) > before);

        // The card belongs to Ana, written by the AI account through her key.
        let cards = db
            .list_cards(ana.id, &cardfile_core::CardFilter::default())
            .unwrap();
        assert_eq!(cards.len(), 1);
        let card = db.card(cards[0].id).unwrap();
        let ai = db.ai_user().unwrap();
        assert_eq!(card.updated_by, Some(ai.id));
        let log = db.activity(Some(card.id), None, 5).unwrap();
        assert_eq!(log[0].via.as_deref(), Some("Claude"));
        assert_eq!(log[0].on_behalf_of, Some(ana.id));
        assert!(db.list_api_keys(ana.id).unwrap()[0].last_used_at.is_some());
        server.stop();
    }
}
