use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDateTime};
use serde::{de::Error, Deserialize, Deserializer, Serializer};

pub const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";
const FORMATS: [&'static str; 19] = [
    "%Y-%m-%d_%H-%M-%S",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%d",
    "%y-%m-%d %H:%M:%S",
    "%y-%m-%d %H:%M",
    "%y-%m-%d",
    "%Y.%m.%d %H:%M:%S",
    "%Y.%m.%d %H:%M",
    "%Y.%m.%d",
    "%y.%m.%d %H:%M:%S",
    "%y.%m.%d %H:%M",
    "%y.%m.%d",
    "%Y%m%d%H%M%S",
    "%Y%m%d%H%M",
    "%Y%m%d",
    "%y%m%d%H%M%S",
    "%y%m%d%H%M",
    "%y%m%d",
];

pub fn serialize<S>(date: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match date {
        None => serializer.serialize_str(""),
        Some(date) => serializer.serialize_str(&format!("{}", date.format(FORMAT))),
    }
}

pub fn deserialize<'a, D>(deserializer: D) -> Result<Option<NaiveDateTime>, D::Error>
where
    D: Deserializer<'a>,
{
    let date = &String::deserialize(deserializer)?;
    if date == "" {
        Ok(None)
    } else {
        NaiveDateTime::parse_from_str(&date, FORMAT)
            .map_err(Error::custom)
            .map(|v| Some(v))
    }
}

pub fn system_to_naive(time: SystemTime) -> NaiveDateTime {
    DateTime::<Local>::from(time).naive_local()
}

pub fn try_parse(input: &str) -> Result<Option<NaiveDateTime>, &str> {
    if input == "" {
        return Ok(None);
    }
    for f in FORMATS.iter() {
        if let Ok(t) = NaiveDateTime::parse_from_str(input, f) {
            return Ok(Some(t));
        }
    }
    Err("Unknown time format, try, e.g., `YYMMDD`")
}
