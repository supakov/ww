//! The current time as an ISO 8601 text in UTC, without a date crate, and the clock types every
//! crate of W# takes: `std::time` natively, the clock of the browser on wasm32, where the ones of
//! `std` panic.

pub use web_time::{Instant, SystemTime, UNIX_EPOCH};

/// `2026-10-14T13:45:10Z` for now, or an empty text when the clock is before 1970.
pub fn now_utc_iso() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| iso_from_unix(d.as_secs()))
        .unwrap_or_default()
}

/// The ISO text of a Unix time (seconds), UTC.
pub fn iso_from_unix(secs: u64) -> String {
    let days = secs / 86_400;
    let rest = secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

/// Days since 1970-01-01 to a civil date (the algorithm of Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(iso_from_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_from_unix(1_792_000_000), "2026-10-14T17:46:40Z");
        assert!(now_utc_iso().starts_with("20"));
    }
}
