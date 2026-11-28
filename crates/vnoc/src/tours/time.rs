//! The time of a record: UTC, whole seconds, `2026-11-23T18:40:02Z` (`docs/26-tours/01-on-disk.md` §4). From `SystemTime`, with no crate for dates.

use std::time::{SystemTime, UNIX_EPOCH};

/// Now, as `2026-11-23T18:40:02Z`.
pub fn now() -> String {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        // a clock set before 1970: the epoch, never a panic
        Err(_) => 0,
    };
    format(seconds)
}

/// `seconds` since 1970-01-01T00:00:00Z as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn format(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day % 3600 / 60,
        of_day % 60
    )
}

/// The seconds since 1970-01-01T00:00:00Z of `YYYY-MM-DDTHH:MM:SSZ`: the other way round of [`format`]. `None` for anything that is not exactly that shape.
pub fn parse(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    if b.len() != 20
        || (b[4], b[7], b[10], b[13], b[16], b[19]) != (b'-', b'-', b'T', b':', b':', b'Z')
    {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<i64> {
        let digits = text.get(from..to)?;
        digits
            .bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| digits.parse().ok())?
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(
        days_from_civil(year, month as u32, day as u32) * 86_400
            + hour * 3600
            + minute * 60
            + second,
    )
}

/// The day (counted from 1970-01-01) of a date: the other way round of [`civil_from_days`].
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = i64::from(if month > 2 { month - 3 } else { month + 9 });
    let day_of_year = (153 * month_from_march + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// How long ago, in the words `where` uses: `12 s ago` under a minute, `5 min ago`, `3 h ago`, `2 days ago`. Each is the whole units passed; a time
/// in the future (a clock that moved) is `0 s ago`.
pub fn ago(seconds: i64) -> String {
    let seconds = seconds.max(0);
    match seconds {
        0..=59 => format!("{seconds} s ago"),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{} h ago", seconds / 3600),
        _ => {
            let days = seconds / 86_400;
            format!("{days} {} ago", if days == 1 { "day" } else { "days" })
        }
    }
}

/// The date of a day counted from 1970-01-01 (day 0), in the proleptic Gregorian calendar: (year, month 1–12, day 1–31).
/// Howard Hinnant's `civil_from_days`: years are counted from 1 March, so the leap day is the last day of a year.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468; // days since 0000-03-01
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097); // 0..=146096
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365; // 0..=399
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // 0..=365, from 1 March
    let month_from_march = (5 * day_of_year + 2) / 153; // 0..=11
    let day = (day_of_year - (153 * month_from_march + 2) / 5 + 1) as u32;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch() {
        assert_eq!(format(0), "1970-01-01T00:00:00Z");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn a_leap_day() {
        assert_eq!(format(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format(1_709_164_800 + 86_399), "2024-02-29T23:59:59Z");
        assert_eq!(format(1_709_164_800 + 86_400), "2024-03-01T00:00:00Z");
        // 2000 was a leap year (a multiple of 400), 2100 is not (a multiple of 100)
        assert_eq!(format(951_825_600), "2000-02-29T12:00:00Z");
        assert_eq!(format(4_107_542_400), "2100-03-01T00:00:00Z");
    }

    #[test]
    fn a_known_time_from_its_seconds() {
        assert_eq!(format(1_795_459_202), "2026-11-23T18:40:02Z");
    }

    #[test]
    fn the_last_second_of_a_year_and_the_first_of_the_next() {
        assert_eq!(format(1_704_067_199), "2023-12-31T23:59:59Z");
        assert_eq!(format(1_704_067_200), "2024-01-01T00:00:00Z");
    }

    #[test]
    fn before_the_epoch_still_makes_a_date() {
        assert_eq!(format(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn a_time_parses_back_to_its_seconds_and_nothing_else_does() {
        for seconds in [
            0,
            1,
            1_709_164_800,
            1_795_459_202,
            4_107_542_400,
            951_825_600,
        ] {
            assert_eq!(parse(&format(seconds)), Some(seconds), "{seconds}");
        }
        for bad in [
            "",
            "yesterday",
            "2026-11-23 18:40:02Z",
            "2026-11-23T18:40:02",
            "2026-13-23T18:40:02Z",
            "2026-11-23T25:40:02Z",
            "+026-11-23T18:40:02Z",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn ago_is_in_whole_units_and_seconds_stay_under_a_minute() {
        let table = [
            (-5, "0 s ago"),
            (0, "0 s ago"),
            (12, "12 s ago"),
            (59, "59 s ago"),
            (60, "1 min ago"),
            (300, "5 min ago"),
            (3599, "59 min ago"),
            (3600, "1 h ago"),
            (3 * 3600 + 59, "3 h ago"),
            (86_399, "23 h ago"),
            (86_400, "1 day ago"),
            (2 * 86_400 + 5, "2 days ago"),
        ];
        for (seconds, want) in table {
            assert_eq!(ago(seconds), want, "{seconds}");
        }
    }

    #[test]
    fn now_has_the_shape_of_a_time_and_is_a_recent_one() {
        let now = now();
        assert_eq!(now.len(), 20, "{now}");
        assert!(now.ends_with('Z') && now.as_bytes()[10] == b'T', "{now}");
        assert!(now.as_str() >= "2026-11-24T00:00:00Z", "{now}");
    }
}
