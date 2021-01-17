use std::cmp::max;
use std::io::Write;

#[allow(dead_code)]
pub struct ProgressBar {
    max: usize,
    current: usize,
    steps: usize,
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
            steps: steps - 1,
        }
    }

    pub fn progress(&mut self) {
        if self.current < self.max {
            self.current += 1;
            if self.current == self.max {
                println!("#");
            } else if self.current * self.steps % self.max < self.steps {
                print!("#");
                std::io::stdout().flush().unwrap();
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
                assert_eq!(*s, count + 1);
            }
        }
    }
}
