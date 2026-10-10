use std::fmt;

use crate::report::format::render;

pub struct Account {
    owner: String,
    cents: i64,
}

impl Account {
    pub fn new(owner: &str) -> Self {
        Account {
            owner: owner.to_owned(),
            cents: 0,
        }
    }

    pub fn deposit(&mut self, cents: i64) {
        self.cents += cents;
    }

    pub fn statement(&self) -> String {
        fn line(label: &str, cents: i64) -> String {
            format!("{label}: {cents}")
        }
        let header = |owner: &str| format!("Account of {owner}");
        render(&[header(&self.owner), line("balance", self.cents)])
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.owner, self.cents)
    }
}

pub trait Audited {
    fn audit_trail(&self) -> Vec<String>;

    fn is_clean(&self) -> bool {
        self.audit_trail().is_empty()
    }
}
