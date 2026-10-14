//! Minimal MCP server with one `echo` tool.
//!
//! ```text
//! cargo run -p cardfile-mcp --example echo_server -- [PORT] [TOKEN]
//! ```
//!
//! Prints the endpoint URL and ready-made client configurations, then serves until Enter is
//! pressed.

use std::sync::Arc;

use cardfile_mcp::{Handler, ServerConfig, ServerInfo, Tool, ToolOutput};
use serde_json::{Value, json};

struct Echo {
    token: String,
}

impl Handler for Echo {
    type Principal = ();

    fn authenticate(&self, token: &str) -> Option<()> {
        (token == self.token).then_some(())
    }

    fn tools(&self, _: &()) -> Vec<Tool> {
        vec![
            Tool::new(
                "echo",
                "Returns the given text unchanged.",
                json!({
                    "type": "object",
                    "properties": { "text": { "type": "string", "description": "Text to echo" } },
                    "required": ["text"],
                    "additionalProperties": false
                }),
            )
            .with_title("Echo")
            .idempotent(),
        ]
    }

    fn call_tool(&self, _: &(), _name: &str, arguments: Value) -> Result<ToolOutput, String> {
        arguments["text"]
            .as_str()
            .map(|text| ToolOutput::Text(text.to_owned()))
            .ok_or_else(|| "\"text\" must be a string".to_owned())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let port = args.next().map(|p| p.parse()).transpose()?.unwrap_or(0);
    let token = args.next().unwrap_or_else(|| "dev-token".to_owned());

    let handle = cardfile_mcp::start(
        ServerConfig::with_port(port),
        ServerInfo::new("echo", env!("CARGO_PKG_VERSION"))
            .with_title("Echo example")
            .with_instructions("Call `echo` to get your text back."),
        Arc::new(Echo {
            token: token.clone(),
        }),
    )?;

    let url = handle.url();
    println!("MCP server listening at {url}\n");
    for (client, snippet) in cardfile_mcp::client_config_snippets(&url, &token) {
        println!("== {client} ==\n{snippet}\n");
    }
    println!("Press Enter to stop.");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    handle.stop();
    Ok(())
}
