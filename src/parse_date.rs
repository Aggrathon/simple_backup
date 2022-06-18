/// This module contains date parsing, serialisation and deserialisation helpers
use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, ParseError};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serializer};

const FORMATS_DT: [&str; 13] = [
    "%Y-%m-%d_%H-%M-%S",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M",
    "%y-%m-%d %H:%M:%S",
    "%y-%m-%d %H:%M",
    "%Y.%m.%d %H:%M:%S",
    "%Y.%m.%d %H:%M",
    "%y.%m.%d %H:%M:%S",
    "%y.%m.%d %H:%M",
    "%Y%m%d%H%M%S",
    "%Y%m%d%H%M",
    "%y%m%d%H%M%S",
    "%y%m%d%H%M",
];
const FORMATS_D: [&str; 6] = [
    "%Y-%m-%d", "%y-%m-%d", "%Y.%m.%d", "%y.%m.%d", "%Y%m%d", "%y%m%d",
];

/// Serialise a Option<NaiveDateTime> (for serde)
pub fn serialize<S>(date: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match date {
        None => serializer.serialize_str(""),
        Some(date) => serializer.serialize_str(&format!("{}", date.format("%Y-%m-%d %H:%M:%S"))),
    }
}

/// Deserialise a Option<NaiveDateTime> (for serde)
pub fn deserialize<'a, D>(deserializer: D) -> Result<Option<NaiveDateTime>, D::Error>
where
    D: Deserializer<'a>,
{
    let date = &String::deserialize(deserializer)?;
    if date.is_empty() {
        Ok(None)
    } else {
        NaiveDateTime::parse_from_str(date, "%Y-%m-%d %H:%M:%S")
            .map_err(Error::custom)
            .map(Some)
    }
}

/// Convert a SystemTime to NaiveDateTime
pub fn system_to_naive(time: SystemTime) -> NaiveDateTime {
    DateTime::<Local>::from(time).naive_local()
}

/// Try parsing a string into a NaiveDateTime
pub fn try_parse(input: &str) -> Result<Option<NaiveDateTime>, &'static str> {
    if input.is_empty() {
        return Ok(None);
    }
    for f in FORMATS_DT.iter() {
        if let Ok(t) = NaiveDateTime::parse_from_str(input, *f) {
            return Ok(Some(t));
        }
    }
    for f in FORMATS_D.iter() {
        if let Ok(t) = NaiveDate::parse_from_str(input, *f) {
            return Ok(Some(t.and_hms(0, 0, 0)));
        }
    }
    Err("Unknown time format, try, e.g., `YYMMDD`")
}

/// Try parsing a backup file name into a NaiveDateTime
pub fn parse_backup_file_name<S: AsRef<str>>(filename: S) -> Result<NaiveDateTime, ParseError> {
    NaiveDateTime::parse_from_str(filename.as_ref(), "backup_%Y-%m-%d_%H-%M-%S.tar.zst")
}

// Encode a NaiveDateTime into a backup file name
pub fn create_backup_file_name(time: NaiveDateTime) -> String {
    format!("{}", time.format("backup_%Y-%m-%d_%H-%M-%S.tar.zst"))
}

/// Get the current time as a NaiveDateTime
pub fn naive_now() -> NaiveDateTime {
    system_to_naive(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use chrono::{Datelike, Timelike};

    use super::{system_to_naive, try_parse};

    #[test]
    fn parse() {
        let now = SystemTime::now();
        let now2 = system_to_naive(now);
        let string = format!("{}-{:02}-{:02}", now2.year(), now2.month(), now2.day());
        let now3 = try_parse(&string).unwrap().unwrap();
        assert_eq!(now2.year(), now3.year());
        assert_eq!(now2.month(), now3.month());
        assert_eq!(now2.day(), now3.day());
        let string = format!("{}{:02}{:02}", now2.year(), now2.month(), now2.day());
        let now3 = try_parse(&string).unwrap().unwrap();
        assert_eq!(now2.year(), now3.year());
        assert_eq!(now2.month(), now3.month());
        assert_eq!(now2.day(), now3.day());
        assert_eq!(try_parse("").unwrap(), None);
        let string = format!(
            "{}.{:02}.{:02} {:02}:{:02}:{:02}",
            now2.year(),
            now2.month(),
            now2.day(),
            now2.hour(),
            now2.minute(),
            now2.second()
        );
        let now3 = try_parse(&string).unwrap().unwrap();
        assert_eq!(now2.year(), now3.year());
        assert_eq!(now2.month(), now3.month());
        assert_eq!(now2.day(), now3.day());
        assert_eq!(now2.hour(), now3.hour());
        assert_eq!(now2.minute(), now3.minute());
        assert_eq!(now2.second(), now3.second());
    }
}
