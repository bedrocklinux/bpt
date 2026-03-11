use crate::{color::Color, error::*, io::BoundedFile, make_display_color};
use std::{fmt::Display, io::Read, time::SystemTime};

/// Seconds since UNIX epoch that a given file was created/updated
///
/// 64-bit little-endian
#[derive(Debug, Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(u64);

/// Convert seconds since UNIX epoch to (year, month, day, hour, minute, second) in UTC.
fn epoch_to_parts(epoch_secs: u64) -> (u64, u8, u8, u8, u8, u8) {
    let secs_per_day: u64 = 86400;
    let days_since_epoch = epoch_secs / secs_per_day;
    let time_of_day = epoch_secs % secs_per_day;

    let hour = (time_of_day / 3600) as u8;
    let min = ((time_of_day % 3600) / 60) as u8;
    let sec = (time_of_day % 60) as u8;

    // O(1) Gregorian conversion from days since Unix epoch.
    // Based on Howard Hinnant's civil_from_days algorithm.
    let z = days_since_epoch as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8; // [1, 31]
    let month = (mp + if mp < 10 { 3 } else { -9 }) as u8; // [1, 12]
    let year = (y + if month <= 2 { 1 } else { 0 }) as u64;

    (year, month, day, hour, min, sec)
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (year, month, day, hour, min, sec) = epoch_to_parts(self.0);
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            year, month, day, hour, min, sec
        )
    }
}

make_display_color!(Timestamp, |s, f| {
    write!(f, "{}{}{}", Color::Timestamp, s, Color::Default)
});

impl Timestamp {
    pub fn from_system_time(time: SystemTime) -> Result<Self, std::time::SystemTimeError> {
        time.duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map(Timestamp)
    }

    pub fn serialize<W: std::io::Write>(&self, w: &mut W) -> Result<(), AnonLocErr> {
        w.write_all(&self.0.to_le_bytes())
            .map_err(AnonLocErr::Write)?;
        Ok(())
    }

    pub fn deserialize(bytes: &[u8; std::mem::size_of::<u64>()]) -> Self {
        Timestamp(u64::from_le_bytes(*bytes))
    }

    pub fn now() -> Result<Self, std::time::SystemTimeError> {
        Self::from_system_time(SystemTime::now())
    }
}

pub trait HandleTimestamp {
    fn strip_timestamp(self) -> Result<(Self, Timestamp), AnonLocErr>
    where
        Self: std::marker::Sized;
}

impl HandleTimestamp for BoundedFile {
    fn strip_timestamp(mut self: BoundedFile) -> Result<(Self, Timestamp), AnonLocErr>
    where
        Self: std::marker::Sized,
    {
        let mut buf = [b'\0'; std::mem::size_of::<u64>()];
        self.read_exact(&mut buf).map_err(AnonLocErr::Read)?;
        let timestamp = Timestamp::deserialize(&buf);
        self.increase_lower_bound_by(buf.len() as u64)?;
        Ok((self, timestamp))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(secs: u64) -> String {
        Timestamp(secs).to_string()
    }

    #[test]
    fn epoch_zero() {
        assert_eq!(ts(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_date() {
        // 2025-02-22T14:32:25Z
        assert_eq!(ts(1740234745), "2025-02-22T14:32:25Z");
    }

    #[test]
    fn leap_year_feb_29_2000() {
        // 2000-02-29T00:00:00Z = 951782400
        assert_eq!(ts(951782400), "2000-02-29T00:00:00Z");
    }

    #[test]
    fn leap_year_feb_29_2024() {
        // 2024-02-29T12:00:00Z = 1709208000
        assert_eq!(ts(1709208000), "2024-02-29T12:00:00Z");
    }

    #[test]
    fn year_boundary() {
        // 2024-12-31T23:59:59Z = 1735689599
        assert_eq!(ts(1735689599), "2024-12-31T23:59:59Z");
        // 2025-01-01T00:00:00Z = 1735689600
        assert_eq!(ts(1735689600), "2025-01-01T00:00:00Z");
    }

    #[test]
    fn end_of_day() {
        // 1970-01-01T23:59:59Z = 86399
        assert_eq!(ts(86399), "1970-01-01T23:59:59Z");
    }

    #[test]
    fn year_2100_plus() {
        // 2100-01-01T00:00:00Z = 4102444800
        assert_eq!(ts(4102444800), "2100-01-01T00:00:00Z");
    }

    #[test]
    fn non_leap_century_2100() {
        // 2100 is NOT a leap year. 2100-03-01T00:00:00Z = 4107542400
        assert_eq!(ts(4107542400), "2100-03-01T00:00:00Z");
        // 2100-02-28T23:59:59Z = 4107542399
        assert_eq!(ts(4107542399), "2100-02-28T23:59:59Z");
    }

    #[test]
    fn leap_day_boundary_2024() {
        // 2024-02-28T23:59:59Z = 1709164799
        assert_eq!(ts(1709164799), "2024-02-28T23:59:59Z");
        // 2024-02-29T00:00:00Z = 1709164800
        assert_eq!(ts(1709164800), "2024-02-29T00:00:00Z");
        // 2024-02-29T23:59:59Z = 1709251199
        assert_eq!(ts(1709251199), "2024-02-29T23:59:59Z");
        // 2024-03-01T00:00:00Z = 1709251200
        assert_eq!(ts(1709251200), "2024-03-01T00:00:00Z");
    }

    #[test]
    fn non_leap_day_boundary_2023() {
        // 2023-02-28T23:59:59Z = 1677628799
        assert_eq!(ts(1677628799), "2023-02-28T23:59:59Z");
        // 2023-03-01T00:00:00Z = 1677628800
        assert_eq!(ts(1677628800), "2023-03-01T00:00:00Z");
    }

    #[test]
    fn four_hundred_year_rule_2400() {
        // 2400 is a leap year. 2400-02-29T00:00:00Z = 13574563200
        assert_eq!(ts(13574563200), "2400-02-29T00:00:00Z");
        // 2400-03-01T00:00:00Z = 13574649600
        assert_eq!(ts(13574649600), "2400-03-01T00:00:00Z");
    }

    #[test]
    fn month_boundary_april_to_may_2025() {
        // 2025-04-30T23:59:59Z = 1746057599
        assert_eq!(ts(1746057599), "2025-04-30T23:59:59Z");
        // 2025-05-01T00:00:00Z = 1746057600
        assert_eq!(ts(1746057600), "2025-05-01T00:00:00Z");
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let original = Timestamp(1735689600); // 2025-01-01T00:00:00Z
        let mut buf = Vec::new();
        original.serialize(&mut buf).unwrap();
        assert_eq!(buf.len(), std::mem::size_of::<u64>());
        let bytes: [u8; 8] = buf.as_slice().try_into().unwrap();
        let parsed = Timestamp::deserialize(&bytes);
        assert_eq!(parsed, original);
    }
}
