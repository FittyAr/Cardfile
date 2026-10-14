//! The HTTP listener (tiny_http) and its worker threads.

use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::McpError;
use crate::protocol::Protocol;
use crate::transport::{Endpoint, HttpRequest};
use crate::types::{Handler, ServerConfig, ServerInfo};

/// How often idle workers re-check the stop flag.
const POLL: Duration = Duration::from_millis(250);
/// Upper bound for waiting until the listening socket is closed on stop.
const CLOSE_WAIT: Duration = Duration::from_secs(2);

/// Starts the MCP server on `127.0.0.1:config.port` and returns immediately; requests are
/// served on `config.threads` background threads until the handle is stopped or dropped.
///
/// # Errors
/// [`McpError::PortInUse`] when the port is taken, [`McpError::Bind`] for other socket errors,
/// [`McpError::InvalidConfig`] for a bad configuration and [`McpError::Start`] when the
/// threads cannot be spawned.
pub fn start<H: Handler>(
    config: ServerConfig,
    info: ServerInfo,
    handler: Arc<H>,
) -> Result<ServerHandle, McpError> {
    config.validate()?;
    let requested = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    let listener = TcpListener::bind(requested).map_err(|source| {
        if source.kind() == ErrorKind::AddrInUse {
            McpError::PortInUse { port: config.port }
        } else {
            McpError::Bind {
                addr: requested,
                source,
            }
        }
    })?;
    let local_addr = listener.local_addr().map_err(|source| McpError::Bind {
        addr: requested,
        source,
    })?;
    let server = tiny_http::Server::from_listener(listener, None)
        .map_err(|e| McpError::Start(e.to_string()))?;
    let server = Arc::new(server);

    let endpoint = Arc::new(Endpoint::new(
        Protocol::new(handler, info, config.tools_ttl_ms),
        config.path.clone(),
        config.max_body,
        config.max_sessions,
        config.session_idle_timeout,
    ));
    let stop = Arc::new(AtomicBool::new(false));

    let mut handle = ServerHandle {
        addr: local_addr,
        path: config.path,
        server: Some(Arc::clone(&server)),
        stop: Arc::clone(&stop),
        workers: Vec::with_capacity(config.threads),
    };
    for n in 0..config.threads {
        let server = Arc::clone(&server);
        let endpoint = Arc::clone(&endpoint);
        let stop = Arc::clone(&stop);
        let worker = std::thread::Builder::new()
            .name(format!("cardfile-mcp-{n}"))
            .spawn(move || worker_loop(&server, &endpoint, &stop));
        match worker {
            Ok(w) => handle.workers.push(w),
            // Dropping the handle stops the workers already started.
            Err(e) => return Err(McpError::Start(e.to_string())),
        }
    }
    tracing::info!(addr = %local_addr, "MCP server listening");
    Ok(handle)
}

fn worker_loop<H: Handler>(server: &tiny_http::Server, endpoint: &Endpoint<H>, stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        match server.recv_timeout(POLL) {
            Ok(Some(request)) => serve(request, endpoint),
            Ok(None) => {}
            Err(e) => {
                tracing::debug!(error = %e, "MCP listener error");
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

fn serve<H: Handler>(mut request: tiny_http::Request, endpoint: &Endpoint<H>) {
    let headers: Vec<(String, String)> = request
        .headers()
        .iter()
        .map(|h| {
            (
                h.field.as_str().as_str().to_ascii_lowercase(),
                h.value.as_str().to_owned(),
            )
        })
        .collect();
    let method = request.method().as_str().to_owned();
    let url = request.url().to_owned();
    let content_length = request.body_length();
    let response = endpoint.handle(&mut HttpRequest {
        method: &method,
        url: &url,
        headers: &headers,
        content_length,
        body: request.as_reader(),
    });
    tracing::debug!(%method, status = response.status, "MCP request");

    let mut out = tiny_http::Response::from_data(response.body).with_status_code(response.status);
    for (name, value) in response.headers {
        if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
            out.add_header(header);
        }
    }
    if let Err(e) = request.respond(out) {
        tracing::debug!(error = %e, "MCP client went away before the response");
    }
}

/// A running MCP server. Stopping it (explicitly or by dropping it) waits for in-flight
/// requests to finish and closes the port.
pub struct ServerHandle {
    addr: SocketAddr,
    path: String,
    server: Option<Arc<tiny_http::Server>>,
    stop: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for ServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerHandle")
            .field("url", &self.url())
            .field("running", &self.server.is_some())
            .finish()
    }
}

impl ServerHandle {
    /// The bound address (useful with port 0).
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The endpoint URL clients connect to, e.g. `http://127.0.0.1:3917/mcp`.
    pub fn url(&self) -> String {
        format!("http://{}{}", self.addr, self.path)
    }

    /// Stops the server and frees the port. Blocks until running tool calls return.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let Some(server) = self.server.take() else {
            return;
        };
        self.stop.store(true, Ordering::Release);
        for _ in 0..self.workers.len() {
            server.unblock();
        }
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                tracing::warn!("MCP worker thread panicked");
            }
        }
        // Last reference: tiny_http closes the listener on its accept thread.
        drop(server);
        wait_until_closed(self.addr);
        tracing::info!(addr = %self.addr, "MCP server stopped");
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// tiny_http drops the listening socket asynchronously on its accept thread; wait (bounded)
/// until connections are refused so the port is reusable when `stop` returns.
fn wait_until_closed(addr: SocketAddr) {
    let deadline = Instant::now() + CLOSE_WAIT;
    while Instant::now() < deadline {
        match TcpStream::connect_timeout(&addr, Duration::from_millis(100)) {
            Ok(stream) => {
                let _ = stream.shutdown(std::net::Shutdown::Both);
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return,
        }
    }
}
