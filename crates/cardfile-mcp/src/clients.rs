//! Ready-to-paste configuration for popular MCP clients.

use serde_json::json;

/// Name under which the server is registered in client configurations.
const SERVER_KEY: &str = "cardfile";

/// Configuration snippets for connecting popular MCP clients to `url` with `token` as the
/// bearer token. Returns `(client name, snippet)` pairs in display order; the application
/// can show each with a copy button.
///
/// The snippets embed the token: treat them as secrets.
pub fn client_config_snippets(url: &str, token: &str) -> Vec<(String, String)> {
    let auth = format!("Bearer {token}");
    let claude_code_cli = format!(
        "claude mcp add --transport http {SERVER_KEY} {url} --header \"Authorization: {auth}\""
    );
    let claude_code_json = json!({
        "mcpServers": { SERVER_KEY: {
            "type": "http",
            "url": url,
            "headers": { "Authorization": auth },
        }}
    });
    // Claude Desktop launches local (stdio) servers from its config file; the `mcp-remote`
    // bridge connects it to an HTTP server. The header is passed through an environment
    // variable because Claude Desktop on Windows does not escape spaces inside `args`.
    let claude_desktop = json!({
        "mcpServers": { SERVER_KEY: {
            "command": "npx",
            "args": [
                "-y", "mcp-remote", url,
                "--header", "Authorization:${AUTH_HEADER}",
                "--transport", "http-only",
                "--allow-http",
            ],
            "env": { "AUTH_HEADER": auth },
        }}
    });
    let cursor = json!({
        "mcpServers": { SERVER_KEY: {
            "url": url,
            "headers": { "Authorization": auth },
        }}
    });
    let vscode = json!({
        "servers": { SERVER_KEY: {
            "type": "http",
            "url": url,
            "headers": { "Authorization": auth },
        }}
    });
    vec![
        ("Claude Code".to_owned(), claude_code_cli),
        (
            "Claude Code (.mcp.json)".to_owned(),
            format!("{claude_code_json:#}"),
        ),
        (
            "Claude Desktop (claude_desktop_config.json)".to_owned(),
            format!("{claude_desktop:#}"),
        ),
        (
            "Cursor (~/.cursor/mcp.json)".to_owned(),
            format!("{cursor:#}"),
        ),
        (
            "VS Code (.vscode/mcp.json)".to_owned(),
            format!("{vscode:#}"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn snippets_are_valid_and_carry_the_token() {
        let url = "http://127.0.0.1:3917/mcp";
        let snippets = client_config_snippets(url, "s3cr\"et");
        assert_eq!(snippets.len(), 5);
        assert!(
            snippets[0]
                .1
                .starts_with("claude mcp add --transport http cardfile")
        );
        for (_, snippet) in &snippets[1..] {
            let v: Value = serde_json::from_str(snippet).unwrap();
            let text = v.to_string();
            assert!(text.contains(url));
            assert!(text.contains("Bearer s3cr\\\"et"));
        }
        let vscode: Value = serde_json::from_str(&snippets[4].1).unwrap();
        assert_eq!(vscode["servers"]["cardfile"]["type"], "http");
    }
}
