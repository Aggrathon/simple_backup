use std::time::SystemTime;

use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use serde::{de::Error, Deserialize, Deserializer, Serializer};

pub const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";
const FORMATS_DT: [&'static str; 13] = [
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
const FORMATS_D: [&'static str; 6] = [
    "%Y-%m-%d", "%y-%m-%d", "%Y.%m.%d", "%y.%m.%d", "%Y%m%d", "%y%m%d",
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

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use chrono::{DateTime, Datelike, Local, Timelike};

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
