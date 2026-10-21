//! Wall-clock maths without an OS: unix time -> civil date and time.

pub const DAYS: [&str; 7] = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
pub const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: i32,
    pub month: u8, // 1-12
    pub day: u8,   // 1-31
    pub weekday: u8, // 0 = Monday
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    /// Local time from unix seconds and the UTC offset.
    pub fn from_unix(unix: i64) -> Self {
        let days = unix.div_euclid(86_400);
        let secs = unix.rem_euclid(86_400);
        // Howard Hinnant's civil_from_days
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + if month <= 2 { 1 } else { 0 }) as i32;
        Self {
            year,
            month,
            day,
            weekday: ((days + 3).rem_euclid(7)) as u8, // 1970-01-01 was a Thursday
            hour: (secs / 3600) as u8,
            minute: (secs / 60 % 60) as u8,
            second: (secs % 60) as u8,
        }
    }

    pub fn day_name(&self) -> &'static str {
        DAYS[self.weekday as usize]
    }

    pub fn month_name(&self) -> &'static str {
        MONTHS[self.month as usize - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        let d = DateTime::from_unix(0);
        assert_eq!((d.year, d.month, d.day, d.weekday), (1970, 1, 1, 3));
        // 2026-10-21 14:05:09 UTC, a Wednesday
        let d = DateTime::from_unix(1_792_591_509);
        assert_eq!((d.year, d.month, d.day, d.weekday, d.hour, d.minute, d.second), (2026, 10, 21, 2, 14, 5, 9));
        // leap day
        let d = DateTime::from_unix(951_782_400);
        assert_eq!((d.year, d.month, d.day), (2000, 2, 29));
    }
}
