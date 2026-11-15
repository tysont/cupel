// ABOUTME: An argument group and its size limit, whose inline tests check at compile time that the group
// ABOUTME: can cross threads and that the limit is positive, beside one test that asserts nothing (ADR-222).

pub const LIMIT: usize = 8;

pub struct ArgGroup {
    pub name: String,
}

impl ArgGroup {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // This test fails to compile if ArgGroup is not Send + Sync.
    #[test]
    fn arg_group_send_sync() {
        fn foo<T: Send + Sync>(_: T) {}
        foo(ArgGroup::new("test"));
    }

    #[test]
    fn the_limit_is_positive() {
        const _: () = assert!(LIMIT > 0);
    }

    #[test]
    fn a_group_builds() {
        ArgGroup::new("test");
    }
}
