//! Wall clock, UTC stamps and the `<time>` grammar.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// `YYYYMMDD-HHMMSS` in UTC.
pub fn utc_stamp(unix_ms: u64) -> String {
    let secs = (unix_ms / 1000) as i64;
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let of_day = secs.rem_euclid(86_400);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// Days since 1970-01-01 to a civil date. Howard Hinnant's algorithm, which avoids
/// pulling in a date crate for one stamp.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

/// `500ms`, `10s` or `5m` to milliseconds.
pub fn parse_time(text: &str) -> Result<u64> {
    let bad = || {
        Error::Usage(format!(
            "{text:?} is not a time; use a whole number with ms, s or m"
        ))
    };
    let (digits, factor) = if let Some(n) = text.strip_suffix("ms") {
        (n, 1)
    } else if let Some(n) = text.strip_suffix('s') {
        (n, 1000)
    } else if let Some(n) = text.strip_suffix('m') {
        (n, 60_000)
    } else {
        return Err(bad());
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    digits
        .parse::<u64>()
        .ok()
        .and_then(|n| n.checked_mul(factor))
        .ok_or_else(bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_stamp_of_a_known_instant() {
        assert_eq!(utc_stamp(1_795_012_502_000), "20261118-143502");
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(951_782_400_000), "20000229-000000");
    }

    #[test]
    fn parse_time_accepts_the_three_units() {
        assert_eq!(parse_time("500ms").unwrap(), 500);
        assert_eq!(parse_time("10s").unwrap(), 10_000);
        assert_eq!(parse_time("5m").unwrap(), 300_000);
    }

    #[test]
    fn parse_time_rejects_everything_else() {
        for text in ["10", "1h", "-1s", "", "s", "1.5s"] {
            assert_eq!(parse_time(text).unwrap_err().code(), "usage", "{text}");
        }
    }
}
