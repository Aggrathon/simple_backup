use std::cmp::max;
use std::io::Write;

pub mod parse_date {
    use chrono::NaiveDateTime;
    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S";
    const FORMATS: [&'static str; 18] = [
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

    pub fn serialize<S>(date: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if date.timestamp() == 0 {
            serializer.serialize_str("")
        } else {
            serializer.serialize_str(&format!("{}", date.format(FORMAT)))
        }
    }

    pub fn deserialize<'a, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
    where
        D: Deserializer<'a>,
    {
        let date = &String::deserialize(deserializer)?;
        if date == "" {
            Ok(NaiveDateTime::from_timestamp(0, 0))
        } else {
            NaiveDateTime::parse_from_str(&date, FORMAT).map_err(Error::custom)
        }
    }

    pub fn try_parse(input: &str) -> Result<NaiveDateTime, &str> {
        if input == "" {
            return Ok(NaiveDateTime::from_timestamp(0, 0));
        }
        for f in FORMATS.iter() {
            if let Ok(t) = NaiveDateTime::parse_from_str(input, f) {
                return Ok(t);
            }
        }
        Err("Unknown date format, try, e.g., `YYMMDD`")
    }
}

#[allow(dead_code)]
pub struct ProgressBar {
    max: usize,
    current: usize,
    steps: usize,
    progress: usize,
}

#[allow(dead_code)]
impl ProgressBar {
    pub fn start(size: usize, steps: usize, title: &str) -> Self {
        let length = title.chars().count();
        let steps = max(length + 4, steps);
        let pad = steps - length;
        for _ in 0..(pad / 2) {
            print!("_");
        }
        print!("{}", title);
        for _ in 0..((pad - 1) / 2 + 1) {
            print!("_");
        }
        print!("\n#");
        std::io::stdout().flush().unwrap();
        Self {
            max: size,
            current: 0,
            steps: steps,
            progress: 1,
        }
    }

    pub fn progress(&mut self) {
        if self.current < self.max {
            self.current += 1;
            let blocks = self.current * self.steps / self.max;
            if blocks > self.progress {
                while blocks > self.progress {
                    print!("#");
                    self.progress += 1;
                }
                if self.current == self.max {
                    println!("");
                } else {
                    std::io::stdout().flush().unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn progress_bar() {
        for n in [333, 500, 100].iter() {
            for s in [20, 27, 63].iter() {
                let mut bar = super::ProgressBar::start(*n, *s, "Backing up files");
                let mut count = 1;
                for _ in 0..*n {
                    bar.progress();
                    if bar.current < bar.max && bar.current * bar.steps % bar.max < bar.steps {
                        count += 1
                    }
                }
                assert_eq!(*s, count);
            }
        }
    }
}
