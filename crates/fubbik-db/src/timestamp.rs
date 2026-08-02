//! Shared wire-serialisation for `timestamp without time zone` columns.
//!
//! Every such column decodes, via sqlx, to a `chrono::NaiveDateTime` — a
//! value with no timezone information at all. `NaiveDateTime`'s default
//! serde `Serialize` impl emits it *without* an offset, e.g.
//! `"2026-06-21T11:40:08.252381"`. JavaScript's `new Date(...)` parses an
//! offset-less ISO string as **local time**, so on the client every one of
//! these timestamps would silently shift by the viewer's UTC offset.
//! Nothing throws; the data is just wrong.
//!
//! Drizzle (the Node backend this is replacing) stores UTC in these same
//! `timestamp without time zone` columns and serialises them as UTC with a
//! trailing `Z`. `UtcTimestamp` reproduces that on the Rust side: it wraps
//! the identical `NaiveDateTime` sqlx already decodes (no schema or query
//! change needed — the column stays `timestamp without time zone`, per the
//! parity report; this is a serialisation fix, not a migration) and treats
//! the stored value as UTC on the way out, emitting RFC 3339 with
//! millisecond precision and a `Z` suffix.
//!
//! Every wire type exposing a `timestamp without time zone` column MUST use
//! this newtype instead of `chrono::NaiveDateTime` directly — a bare
//! `NaiveDateTime` field silently reintroduces the bug with no compiler
//! warning, since it still implements `Serialize` on its own.

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};

// `#[sqlx(transparent)]` on the `Type` derive generates `Type`, `Encode`,
// *and* `Decode` impls that all delegate to `NaiveDateTime`'s — deriving
// `Encode`/`Decode` again separately would conflict with those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(transparent)]
pub struct UtcTimestamp(pub NaiveDateTime);

impl UtcTimestamp {
    /// Reinterprets the stored naive value as UTC (never converts — the
    /// column already holds a UTC instant with the offset stripped, so
    /// there is no timezone math to do here, only labelling).
    pub fn to_utc(self) -> DateTime<Utc> {
        DateTime::<Utc>::from_naive_utc_and_offset(self.0, Utc)
    }
}

impl From<NaiveDateTime> for UtcTimestamp {
    fn from(naive: NaiveDateTime) -> Self {
        Self(naive)
    }
}

impl From<UtcTimestamp> for NaiveDateTime {
    fn from(ts: UtcTimestamp) -> Self {
        ts.0
    }
}

/// Emits e.g. `"2026-06-21T11:40:08.252Z"` — RFC 3339, millisecond
/// precision, trailing `Z` — matching Node/Drizzle's wire format exactly.
impl serde::Serialize for UtcTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_utc().to_rfc3339_opts(SecondsFormat::Millis, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_as_utc_iso8601_with_trailing_z_and_millis() {
        let naive =
            NaiveDateTime::parse_from_str("2026-06-21 11:40:08.252381", "%Y-%m-%d %H:%M:%S%.f")
                .unwrap();
        let ts = UtcTimestamp(naive);

        let json = serde_json::to_string(&ts).unwrap();
        assert!(
            json.ends_with("Z\""),
            "expected a trailing Z before the closing quote, got {json}"
        );
        assert_eq!(json, "\"2026-06-21T11:40:08.252Z\"");
    }

    /// The precision reduction (micros -> millis) must not change *which
    /// instant* is represented — round-tripping the emitted string back
    /// through a UTC-aware parser must land on the same instant modulo the
    /// sub-millisecond remainder that millisecond formatting intentionally
    /// discards.
    #[test]
    fn round_trips_to_the_same_instant() {
        let naive =
            NaiveDateTime::parse_from_str("2026-06-21 11:40:08.252", "%Y-%m-%d %H:%M:%S%.f")
                .unwrap();
        let ts = UtcTimestamp(naive);

        let json = serde_json::to_string(&ts).unwrap();
        let s: String = serde_json::from_str(&json).unwrap();
        let parsed = DateTime::parse_from_rfc3339(&s).unwrap();

        assert_eq!(parsed.with_timezone(&Utc), ts.to_utc());
    }
}
