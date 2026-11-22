/// Milliseconds since the Unix epoch (UTC). Stored as `BIGINT` in the database.
pub type Millis = i64;

/// Current time in milliseconds since the Unix epoch.
pub fn now_millis() -> Millis {
    jiff::Timestamp::now().as_millisecond()
}
