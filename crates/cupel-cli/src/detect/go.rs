// ABOUTME: Go roots (`go.mod`, `go.work`) and their default commands over every package of the
// ABOUTME: module, or of each module a `go.work` uses (ADR-021).
use cupel_core::syntax::{DefaultCommands, ProjectRoot};

use super::{Candidate, Ecosystem};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["go.work", "go.mod"],
    skip_dirs: &["vendor"],
    describe,
};

fn describe(candidate: &Candidate) -> ProjectRoot {
    let packages = candidate
        .read("go.work")
        .map(|work| used_modules(&work))
        .filter(|modules| !modules.is_empty())
        .map_or_else(
            || "./...".to_owned(),
            |modules| {
                modules
                    .iter()
                    .map(|module| format!("./{}/...", module.trim_start_matches("./")))
                    .collect::<Vec<_>>()
                    .join(" ")
            },
        );
    candidate.root(
        "go",
        DefaultCommands {
            test: Some(format!("go test -json {packages}")),
            coverage: Some(format!("go test -coverprofile={{out}} {packages}")),
            bench: Some(format!("go test -run=^$ -bench=. {packages}")),
        },
    )
}

/// The directories a `go.work` file's `use` directives name, in order: `use ./a` or a
/// parenthesized block of one directory per line. Comments are ignored.
fn used_modules(work: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let mut in_block = false;
    for line in work.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if in_block {
            if line == ")" {
                in_block = false;
            } else if !line.is_empty() {
                modules.push(line.to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("use") {
            match rest.trim() {
                "(" => in_block = true,
                "" => {}
                module => modules.push(module.to_owned()),
            }
        }
    }
    modules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_directives_list_modules_in_order() {
        let work = "go 1.23\n\nuse ./single // one\n\nuse (\n\t./a\n\n\t./b // two\n)\n";
        assert_eq!(used_modules(work), ["./single", "./a", "./b"]);
        assert!(used_modules("go 1.23\n").is_empty());
    }
}
