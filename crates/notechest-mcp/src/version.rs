//! MCP protocol revisions understood by this server.

/// A Model Context Protocol revision this server implements.
///
/// Revisions up to [`ProtocolVersion::V2025_11_25`] are *legacy*: the client opens a session
/// with an `initialize` handshake and the Streamable HTTP transport tracks it with the
/// `Mcp-Session-Id` header. [`ProtocolVersion::V2026_07_28`] is *modern*: there is no
/// handshake or session and every request carries its protocol version in `_meta`. The server
/// is dual-era and serves both on the same endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtocolVersion {
    /// First Streamable HTTP revision. The only one that allows JSON-RPC batches.
    V2025_03_26,
    /// Adds `title` fields, `structuredContent`, `outputSchema` and the
    /// `MCP-Protocol-Version` header; removes batching.
    V2025_06_18,
    /// Last revision with the `initialize` handshake and sessions.
    V2025_11_25,
    /// Stateless revision: per-request `_meta`, `server/discover`, `resultType`.
    V2026_07_28,
}

impl ProtocolVersion {
    /// Every supported revision, newest first.
    pub const ALL: [ProtocolVersion; 4] = [
        Self::V2026_07_28,
        Self::V2025_11_25,
        Self::V2025_06_18,
        Self::V2025_03_26,
    ];

    /// The newest supported revision.
    pub const LATEST: Self = Self::V2026_07_28;

    /// The newest revision that uses the `initialize` handshake. `initialize` requests that ask
    /// for an unknown (or modern) version are answered with this one.
    pub const LATEST_LEGACY: Self = Self::V2025_11_25;

    /// The wire form, e.g. `"2025-11-25"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V2025_03_26 => "2025-03-26",
            Self::V2025_06_18 => "2025-06-18",
            Self::V2025_11_25 => "2025-11-25",
            Self::V2026_07_28 => "2026-07-28",
        }
    }

    /// Parses the wire form; `None` for revisions this server does not implement.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.as_str() == value)
    }

    /// `true` for stateless revisions that carry the version in every request.
    pub fn is_modern(self) -> bool {
        self >= Self::V2026_07_28
    }

    /// Whether `title`, `outputSchema` and `structuredContent` exist in this revision.
    pub(crate) fn has_titles(self) -> bool {
        self >= Self::V2025_06_18
    }

    /// Whether a POST body may be a JSON-RPC batch.
    pub(crate) fn allows_batches(self) -> bool {
        self == Self::V2025_03_26
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Wire forms of every supported revision, newest first.
pub fn supported_versions() -> Vec<&'static str> {
    ProtocolVersion::ALL.iter().map(|v| v.as_str()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_orders() {
        for v in ProtocolVersion::ALL {
            assert_eq!(ProtocolVersion::parse(v.as_str()), Some(v));
        }
        assert_eq!(ProtocolVersion::parse("2024-11-05"), None);
        assert!(ProtocolVersion::LATEST.is_modern());
        assert!(!ProtocolVersion::LATEST_LEGACY.is_modern());
        assert!(ProtocolVersion::V2025_03_26.allows_batches());
        assert!(!ProtocolVersion::V2025_06_18.allows_batches());
        assert!(!ProtocolVersion::V2025_03_26.has_titles());
    }
}
