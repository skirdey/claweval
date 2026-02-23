use crate::checks::CheckOutcome;
use std::sync::Mutex;

pub struct Printer {
    use_color: bool,
    lock: Mutex<()>,
}

impl Printer {
    pub fn new() -> Self {
        // Disable color when NO_COLOR is set or TERM=dumb.
        let no_color = std::env::var("NO_COLOR").is_ok();
        let dumb = std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false);
        Self {
            use_color: !no_color && !dumb,
            lock: Mutex::new(()),
        }
    }

    fn green(&self, s: &str) -> String {
        if self.use_color {
            format!("\x1b[32m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    fn red(&self, s: &str) -> String {
        if self.use_color {
            format!("\x1b[31m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    fn bold(&self, s: &str) -> String {
        if self.use_color {
            format!("\x1b[1m{}\x1b[0m", s)
        } else {
            s.to_string()
        }
    }

    pub fn episode_start(&self, id: &str, desc: Option<&str>) {
        let _g = self.lock.lock().unwrap();
        match desc {
            Some(d) => eprintln!("{} {} — {}", self.bold("Episode"), id, d),
            None => eprintln!("{} {}", self.bold("Episode"), id),
        }
    }

    pub fn run_result(
        &self,
        _ep_id: &str,
        run_index: u32,
        pass: bool,
        checks: &[CheckOutcome],
    ) {
        let _g = self.lock.lock().unwrap();
        let mark = if pass {
            self.green("\u{2713}")
        } else {
            self.red("\u{2717}")
        };
        eprintln!("  {} run #{}", mark, run_index + 1);
        if !pass {
            for c in checks.iter().filter(|c| !c.pass) {
                eprintln!("    [{}] {}", c.check_type, c.details);
            }
        }
    }

    pub fn suite_summary(&self, total: u32, passed: u32, duration_ms: u128) {
        let _g = self.lock.lock().unwrap();
        let rate = if total > 0 {
            passed as f64 / total as f64 * 100.0
        } else {
            0.0
        };
        let summary = format!(
            "{}/{} passed ({:.1}%) in {}ms",
            passed, total, rate, duration_ms
        );
        eprintln!();
        if passed == total {
            eprintln!("{}", self.green(&summary));
        } else {
            eprintln!("{}", self.red(&summary));
        }
    }
}
