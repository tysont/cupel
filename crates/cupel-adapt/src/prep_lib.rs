// ABOUTME: A tools.deps project's own `:deps/prep-lib` (ADR-230): the build its `deps.edn` declares it needs before use,
// ABOUTME: read from the file, and run as `clojure -T<alias> <fn>` before a Clojure run when its `:ensure` directory is missing.
use std::path::Path;

use cupel_core::{Cancellation, Error, Process, Result, output_tail};

/// The manifest that declares it.
const MANIFEST: &str = "deps.edn";

/// The top-level key of the declaration.
const KEY: &str = ":deps/prep-lib";

/// A project's `:deps/prep-lib`: `clojure -T<alias> <function>` prepares it, and `ensure`, a
/// directory relative to the project, exists once it is prepared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prep {
    /// The alias keyword, with its colon, such as `:build`.
    pub alias: String,
    /// The function symbol, resolved in the alias's `:ns-default` when unqualified.
    pub function: String,
    pub ensure: String,
}

impl Prep {
    /// The command that prepares the project: `clojure -T:build compile-java`.
    pub fn command(&self) -> Vec<String> {
        vec![
            "clojure".to_owned(),
            format!("-T{}", self.alias),
            self.function.clone(),
        ]
    }

    /// Whether the project at `dir` is prepared: its `ensure` directory exists.
    pub fn done(&self, dir: &Path) -> bool {
        dir.join(&self.ensure).is_dir()
    }
}

/// The `:deps/prep-lib` the `deps.edn` in `dir` declares; `None` without the file, without the
/// key in its top-level map, or when the declaration lacks a keyword `:alias`, a symbol `:fn`,
/// or a string `:ensure`.
pub fn declared(dir: &Path) -> Option<Prep> {
    let text = std::fs::read_to_string(dir.join(MANIFEST)).ok()?;
    parse(&text)
}

/// Runs the project's declared prep in `dir` when it is not done; a prep that fails is an error
/// carrying its command and the tail of its output.
pub(crate) fn prepare(dir: &Path, cancel: &Cancellation) -> Result<()> {
    let Some(prep) = declared(dir).filter(|prep| !prep.done(dir)) else {
        return Ok(());
    };
    let command = prep.command();
    let captured = Process::new(&command[0], dir)
        .args(command[1..].iter().cloned())
        .merged()
        .run(cancel)?;
    if captured.exit_code == Some(0) {
        return Ok(());
    }
    Err(Error::Process {
        program: command[0].clone(),
        message: format!(
            "`{}`, the build deps.edn's {KEY} declares, did not prepare {}: {}",
            command.join(" "),
            prep.ensure,
            output_tail(&captured.stdout).unwrap_or_default()
        ),
    })
}

/// A token of EDN text, as far as finding the declaration needs: an opening or closing
/// delimiter, a string's contents, or any other atom (keyword, symbol, number).
#[derive(Debug, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Text(String),
    Atom(String),
}

/// The declaration in `text`, the contents of a `deps.edn`.
fn parse(text: &str) -> Option<Prep> {
    let tokens = tokens(text);
    let mut depth = 0usize;
    let mut at = None;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Open => depth += 1,
            Token::Close => depth = depth.saturating_sub(1),
            Token::Atom(atom) if depth == 1 && atom == KEY => {
                at = Some(index + 1);
                break;
            }
            _ => {}
        }
    }
    let entries = map_at(&tokens, at?)?;
    let value = |key: &str| {
        entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    };
    let atom = |key: &str| match value(key)? {
        Token::Atom(atom) => Some(atom.clone()),
        _ => None,
    };
    let alias = atom(":alias").filter(|alias| alias.starts_with(':'))?;
    let function = atom(":fn").filter(|function| !function.starts_with(':'))?;
    let Some(Token::Text(ensure)) = value(":ensure") else {
        return None;
    };
    Some(Prep {
        alias,
        function,
        ensure: ensure.clone(),
    })
}

/// The keys and values of the map opening at `tokens[start]`, each value a single token (a
/// nested value is skipped); `None` when no map opens there.
fn map_at(tokens: &[Token], start: usize) -> Option<Vec<(&str, &Token)>> {
    if tokens.get(start) != Some(&Token::Open) {
        return None;
    }
    let mut entries = Vec::new();
    let mut key: Option<&str> = None;
    let mut index = start + 1;
    while let Some(token) = tokens.get(index) {
        match token {
            Token::Close => return Some(entries),
            Token::Open => {
                index = skip(tokens, index);
                key = None;
                continue;
            }
            token => match key.take() {
                None => key = Some(text_of(token)),
                Some(name) => entries.push((name, token)),
            },
        }
        index += 1;
    }
    None
}

/// The index after the form opening at `tokens[start]`.
fn skip(tokens: &[Token], start: usize) -> usize {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Token::Open => depth += 1,
            Token::Close => {
                depth -= 1;
                if depth == 0 {
                    return index + 1;
                }
            }
            _ => {}
        }
    }
    tokens.len()
}

fn text_of(token: &Token) -> &str {
    match token {
        Token::Text(text) | Token::Atom(text) => text,
        Token::Open | Token::Close => "",
    }
}

/// The tokens of `text`: `;` comments and commas are blank, `{[(` open and `}])` close (a `#{`
/// set opens too), and a string keeps its escaped characters.
fn tokens(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ';' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' | '[' | '(' => tokens.push(Token::Open),
            '}' | ']' | ')' => tokens.push(Token::Close),
            '"' => {
                let mut string = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => string.extend(chars.next()),
                        c => string.push(c),
                    }
                }
                tokens.push(Token::Text(string));
            }
            c if c.is_whitespace() || c == ',' => {}
            c => {
                let mut atom = String::from(c);
                while let Some(next) = chars.peek() {
                    if next.is_whitespace() || ",;\"{}[]()".contains(*next) {
                        break;
                    }
                    atom.extend(chars.next());
                }
                if atom == "#" && chars.peek() == Some(&'{') {
                    continue;
                }
                tokens.push(Token::Atom(atom));
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::{Prep, parse};

    fn prep(alias: &str, function: &str, ensure: &str) -> Prep {
        Prep {
            alias: alias.to_owned(),
            function: function.to_owned(),
            ensure: ensure.to_owned(),
        }
    }

    /// clj-commons/clj-yaml's declaration, among its other keys and comments.
    #[test]
    fn a_top_level_prep_lib_is_read_with_its_alias_function_and_directory() {
        let text = r#";; the library
{:paths ["src/clojure" "target/classes"] ; compiled Java
 :deps {org.clojure/clojure {:mvn/version "1.12.3"}}
 :deps/prep-lib {:alias :build, :fn compile-java
                 :ensure "target/classes"}
 :aliases {:build {:ns-default build}}}"#;
        assert_eq!(
            parse(text),
            Some(prep(":build", "compile-java", "target/classes"))
        );
        assert_eq!(
            parse(r#"{:deps/prep-lib {:ensure "out" :alias :b :fn build/prep}}"#)
                .map(|prep| prep.command()),
            Some(vec![
                "clojure".to_owned(),
                "-T:b".to_owned(),
                "build/prep".to_owned()
            ])
        );
    }

    #[test]
    fn a_missing_nested_or_incomplete_prep_lib_is_none() {
        let texts = [
            r#"{:paths ["src"]}"#,
            r#"{:aliases {:x {:deps/prep-lib {:alias :b :fn f :ensure "t"}}}}"#,
            r#"{:deps/prep-lib {:alias :b :fn f}}"#,
            r#"{:deps/prep-lib {:alias b :fn f :ensure "t"}}"#,
            r#"{:deps/prep-lib {:alias :b :fn f :ensure t}}"#,
            r#"{:deps/prep-lib "not a map"}"#,
            "",
        ];
        assert_eq!(texts.map(parse), [None, None, None, None, None, None, None]);
    }
}
