// ABOUTME: A command builder that checks its own configuration: `debug_assert` panics on a command with no
// ABOUTME: name, as clap's `Command::debug_assert` does, and `assert_named` checks one name.

pub struct Command {
    name: String,
}

impl Command {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
        }
    }

    pub fn debug_assert(&self) {
        assert!(!self.name.is_empty(), "a command needs a name");
    }

    pub fn assert_named(&self, name: &str) {
        assert_eq!(self.name, name);
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
