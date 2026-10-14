// ABOUTME: JavaScript and TypeScript module resolution for `ModuleGraph`: Node's rules for relative paths,
// ABOUTME: package `imports`/`exports`/`main`, workspace packages, builtins, and tsconfig `paths`/`baseUrl`.
//!
//! Queries find `import`, `export … from`, `require(…)`, `import(…)`, and TypeScript's
//! `import x = require(…)`; resolving a specifier needs manifests and the file system, which a
//! query cannot see, so this part of the language is hand-written. A specifier resolves by the
//! first rule that applies:
//!
//! 1. **Relative** (`./a`, `../a`, `.`, `..`): the path from the importing file's directory,
//!    probed as below. These edges are `relative`; no other edge is.
//! 2. **Subpath import** (`#a/b`): the nearest `package.json` above the importing file whose
//!    `imports` map has a matching key (exact, or a pattern with one `*`).
//! 3. **Builtin** (`node:fs`, `fs/promises`): `External` with package `node:<module>`.
//! 4. **`paths`**: the nearest `tsconfig.json` (or `jsconfig.json`) above the importing file;
//!    the longest matching `compilerOptions.paths` pattern, each replacement tried in order
//!    relative to `baseUrl` (else the config's directory). A matching pattern with no file is
//!    `Unresolved`.
//! 5. **`baseUrl`**: the specifier below `baseUrl`, when such a file exists.
//! 6. **Workspace package**: a bare specifier naming a package found in the repository (the
//!    root manifest's `workspaces`, or the nearest `package.json` above a file in scope): its
//!    `exports` (subpath keys, `*` patterns, and conditions preferring `import`, `require`,
//!    `node`, `default`, then `types`), else `main`, else `index`. No such file is `Unresolved`.
//! 7. **Any other bare specifier**: `External`, named by its package (`@scope/name` or `name`).
//!
//! URLs and absolute paths are `Unresolved`. Probing a path tries the file itself, then (for
//! TypeScript) the TypeScript file a `.js`/`.jsx`/`.mjs`/`.cjs` name stands for, then each
//! extension appended, then a directory's `package.json` `main` and its `index` file. TypeScript
//! files try TypeScript extensions first, JavaScript files JavaScript ones. Edges are grouped
//! by the package directory containing the importing file (`.` for files outside every
//! package); every discovered package is listed.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use cupel_core::syntax::{ImportEdge, ImportTarget, ModuleGraph, RootImports};
use serde_json::Value;

use crate::imports::{Imports, RawImport, join};

/// Node's builtin modules, importable with or without `node:`.
const BUILTINS: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

const JAVASCRIPT_EXTENSIONS: &[&str] =
    &[".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx", ".mts", ".cts"];
const TYPESCRIPT_EXTENSIONS: &[&str] =
    &[".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"];

/// The TypeScript sources a JavaScript file name stands for in TypeScript's resolution.
const TYPESCRIPT_FOR: &[(&str, &[&str])] = &[
    (".js", &[".ts", ".tsx"]),
    (".jsx", &[".tsx"]),
    (".mjs", &[".mts"]),
    (".cjs", &[".cts"]),
];

/// Export conditions, most preferred first.
const CONDITIONS: &[&str] = &["import", "require", "node", "default", "types"];

/// Resolves the JavaScript files in scope.
pub fn javascript(imports: &Imports<'_>) -> ModuleGraph {
    graph(imports, false)
}

/// Resolves the TypeScript files in scope.
pub fn typescript(imports: &Imports<'_>) -> ModuleGraph {
    graph(imports, true)
}

fn graph(imports: &Imports<'_>, typescript: bool) -> ModuleGraph {
    let resolver = Resolver {
        imports,
        typescript,
        manifests: RefCell::new(HashMap::new()),
    };
    let packages = resolver.packages();
    let mut roots: BTreeMap<String, Vec<ImportEdge>> = packages
        .iter()
        .map(|package| (package.dir.clone(), Vec::new()))
        .collect();
    for file in imports.scope() {
        let group = package_of(&packages, file).map_or(".", |p| p.dir.as_str());
        let edges = roots.entry(group.to_owned()).or_default();
        for import in imports.of(file).imports {
            edges.push(resolver.edge(&packages, file, &import));
        }
    }
    ModuleGraph {
        roots: roots
            .into_iter()
            .map(|(root, mut edges)| {
                edges.sort_by(|a, b| {
                    (&a.from, a.line, &a.specifier).cmp(&(&b.from, b.line, &b.specifier))
                });
                RootImports { root, edges }
            })
            .collect(),
    }
}

/// A `package.json` found in the repository.
#[derive(Debug, Clone)]
struct Package {
    dir: String,
    name: Option<String>,
}

struct Resolver<'a, 'b> {
    imports: &'a Imports<'b>,
    typescript: bool,
    /// Parsed JSON files by path; `None` when missing or invalid.
    manifests: RefCell<HashMap<String, Option<Value>>>,
}

impl Resolver<'_, '_> {
    fn edge(&self, packages: &[Package], from: &str, import: &RawImport) -> ImportEdge {
        let specifier = import.specifier.clone();
        let relative = is_relative(&specifier);
        ImportEdge {
            from: from.to_owned(),
            target: self.resolve(packages, from, &specifier),
            specifier,
            relative,
            line: import.line,
            edge_kind: import.kind,
        }
    }

    fn resolve(&self, packages: &[Package], from: &str, specifier: &str) -> ImportTarget {
        let dir = parent(from);
        let found = |path: Option<String>| {
            path.map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path })
        };
        if is_relative(specifier) {
            return found(self.probe(&normalize(&join(dir, specifier))));
        }
        if specifier.contains("://") || specifier.starts_with('/') {
            return ImportTarget::Unresolved;
        }
        if specifier.starts_with('#') {
            return found(self.subpath_import(dir, specifier));
        }
        if let Some(builtin) = builtin(specifier) {
            return ImportTarget::External {
                package: format!("node:{builtin}"),
            };
        }
        if let Some(target) = self.configured(dir, specifier) {
            return target;
        }
        let name = package_name(specifier);
        if let Some(package) = packages.iter().find(|p| p.name.as_deref() == Some(name)) {
            let subpath = format!(".{}", &specifier[name.len()..]);
            return found(self.package_entry(&package.dir, &subpath));
        }
        ImportTarget::External {
            package: name.to_owned(),
        }
    }

    /// Resolution through the nearest tsconfig or jsconfig: `paths`, then `baseUrl`. `None`
    /// when neither applies.
    fn configured(&self, dir: &str, specifier: &str) -> Option<ImportTarget> {
        let (config_dir, config) = self.nearest(dir, &["tsconfig.json", "jsconfig.json"])?;
        let options = config.get("compilerOptions")?;
        let base = options
            .get("baseUrl")
            .and_then(Value::as_str)
            .map(|base| normalize(&join(&config_dir, base)));
        let paths_base = base.clone().unwrap_or_else(|| config_dir.clone());
        if let Some(paths) = options.get("paths").and_then(Value::as_object) {
            let best = paths
                .iter()
                .filter_map(|(pattern, targets)| {
                    let star = match_pattern(pattern, specifier)?;
                    Some((pattern.find('*').unwrap_or(pattern.len()), star, targets))
                })
                .max_by_key(|(prefix, ..)| *prefix);
            if let Some((_, star, targets)) = best {
                let file = targets
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .find_map(|target| {
                        self.probe(&normalize(&join(&paths_base, &target.replace('*', &star))))
                    });
                return Some(
                    file.map_or(ImportTarget::Unresolved, |path| ImportTarget::File { path }),
                );
            }
        }
        let path = self.probe(&normalize(&join(base.as_deref()?, specifier)))?;
        Some(ImportTarget::File { path })
    }

    /// `#a/b` through the `imports` map of the nearest manifest above `dir` that has one.
    fn subpath_import(&self, dir: &str, specifier: &str) -> Option<String> {
        let mut current = dir.to_owned();
        loop {
            if let Some(map) = self
                .json(&join(&current, "package.json"))
                .and_then(|m| m.get("imports").cloned())
            {
                let target = subpath_target(&map, specifier)?;
                return self.probe(&normalize(&join(&current, &target)));
            }
            current = parent_dir(&current)?;
        }
    }

    /// The file a package directory provides for `subpath` (`.` or `./a/b`).
    fn package_entry(&self, dir: &str, subpath: &str) -> Option<String> {
        let manifest = self.json(&join(dir, "package.json"));
        if let Some(exports) = manifest.as_ref().and_then(|m| m.get("exports")) {
            let map = if exports
                .as_object()
                .is_some_and(|o| o.keys().any(|k| k.starts_with('.')))
            {
                exports.clone()
            } else {
                serde_json::json!({ ".": exports })
            };
            let target = subpath_target(&map, subpath)?;
            return self.probe(&normalize(&join(dir, &target)));
        }
        if subpath == "." {
            if let Some(main) = manifest
                .as_ref()
                .and_then(|m| m.get("main"))
                .and_then(Value::as_str)
                && let Some(path) = self.probe(&normalize(&join(dir, main)))
            {
                return Some(path);
            }
            return self.index(dir);
        }
        self.probe(&normalize(&join(dir, subpath)))
    }

    /// The file `path` names: itself, its TypeScript source, with an extension, or a
    /// directory's `main` or `index`.
    fn probe(&self, path: &str) -> Option<String> {
        if self.imports.is_file(path) {
            return Some(path.to_owned());
        }
        if self.typescript {
            for (js, sources) in TYPESCRIPT_FOR {
                if let Some(stem) = path.strip_suffix(js) {
                    for source in *sources {
                        let candidate = format!("{stem}{source}");
                        if self.imports.is_file(&candidate) {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        if let Some(found) = self.with_extension(path) {
            return Some(found);
        }
        if self.imports.is_dir(path) {
            if let Some(main) = self
                .json(&join(path, "package.json"))
                .and_then(|m| m.get("main").and_then(Value::as_str).map(str::to_owned))
                && let Some(found) = self.probe(&normalize(&join(path, &main)))
            {
                return Some(found);
            }
            return self.index(path);
        }
        None
    }

    fn with_extension(&self, path: &str) -> Option<String> {
        self.extensions()
            .iter()
            .map(|extension| format!("{path}{extension}"))
            .find(|candidate| self.imports.is_file(candidate))
    }

    fn index(&self, dir: &str) -> Option<String> {
        self.with_extension(&join(dir, "index"))
    }

    fn extensions(&self) -> &'static [&'static str] {
        if self.typescript {
            TYPESCRIPT_EXTENSIONS
        } else {
            JAVASCRIPT_EXTENSIONS
        }
    }

    /// The nearest of `names` in `dir` or above it, with its directory.
    fn nearest(&self, dir: &str, names: &[&str]) -> Option<(String, Value)> {
        let mut current = dir.to_owned();
        loop {
            for name in names {
                if let Some(config) = self.json(&join(&current, name)) {
                    return Some((current, config));
                }
            }
            current = parent_dir(&current)?;
        }
    }

    /// A JSON file (comments and trailing commas allowed, as tsconfig permits), cached.
    fn json(&self, path: &str) -> Option<Value> {
        self.manifests
            .borrow_mut()
            .entry(path.to_owned())
            .or_insert_with(|| {
                let text = self.imports.read(path)?;
                serde_json::from_str(&strip_jsonc(&text)).ok()
            })
            .clone()
    }

    /// The root package, its workspaces, and the nearest package above each file in scope.
    fn packages(&self) -> Vec<Package> {
        let mut dirs: BTreeSet<String> = BTreeSet::new();
        if let Some(root) = self.json("package.json") {
            dirs.insert(".".to_owned());
            let workspaces = root
                .get("workspaces")
                .map(|w| w.get("packages").unwrap_or(w))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for pattern in workspaces.iter().filter_map(Value::as_str) {
                for dir in self.imports.expand(".", pattern) {
                    if self.imports.is_file(&join(&dir, "package.json")) {
                        dirs.insert(dir);
                    }
                }
            }
        }
        for file in self.imports.scope() {
            let mut current = parent(file).to_owned();
            loop {
                if self.imports.is_file(&join(&current, "package.json")) {
                    dirs.insert(if current.is_empty() {
                        ".".to_owned()
                    } else {
                        current
                    });
                    break;
                }
                match parent_dir(&current) {
                    Some(up) => current = up,
                    None => break,
                }
            }
        }
        dirs.into_iter()
            .map(|dir| Package {
                name: self
                    .json(&join(&dir, "package.json"))
                    .and_then(|m| m.get("name").and_then(Value::as_str).map(str::to_owned)),
                dir,
            })
            .collect()
    }
}

/// The target a subpath map (`exports` or `imports`) gives `key`: an exact key, else the
/// pattern with the longest prefix before its `*`, with conditions resolved.
fn subpath_target(map: &Value, key: &str) -> Option<String> {
    let object = map.as_object()?;
    if let Some(target) = object.get(key) {
        return conditional(target, None);
    }
    object
        .iter()
        .filter_map(|(pattern, target)| {
            let star = match_pattern(pattern, key).filter(|_| pattern.contains('*'))?;
            Some((pattern.find('*').unwrap_or(0), star, target))
        })
        .max_by_key(|(prefix, ..)| *prefix)
        .and_then(|(_, star, target)| conditional(target, Some(&star)))
}

/// A target string, the first entry of an array that has one, or the preferred condition of
/// a conditions object; `*` replaced by `star`.
fn conditional(target: &Value, star: Option<&str>) -> Option<String> {
    match target {
        Value::String(path) => Some(star.map_or_else(|| path.clone(), |s| path.replace('*', s))),
        Value::Array(choices) => choices.iter().find_map(|c| conditional(c, star)),
        Value::Object(conditions) => CONDITIONS
            .iter()
            .find_map(|condition| conditional(conditions.get(*condition)?, star)),
        _ => None,
    }
}

/// What `*` in `pattern` stands for when `pattern` matches `text` (empty for an exact match).
fn match_pattern(pattern: &str, text: &str) -> Option<String> {
    match pattern.split_once('*') {
        None => (pattern == text).then(String::new),
        Some((prefix, suffix)) => text
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .map(str::to_owned),
    }
}

fn is_relative(specifier: &str) -> bool {
    matches!(specifier, "." | "..") || specifier.starts_with("./") || specifier.starts_with("../")
}

/// The builtin module a specifier names (`node:fs/promises` and `fs/promises` are `fs`).
fn builtin(specifier: &str) -> Option<&str> {
    let (explicit, rest) = match specifier.strip_prefix("node:") {
        Some(rest) => (true, rest),
        None => (false, specifier),
    };
    let module = rest.split('/').next().unwrap_or(rest);
    (explicit || BUILTINS.contains(&module)).then_some(module)
}

/// The package a bare specifier names: `@scope/name` or `name`.
fn package_name(specifier: &str) -> &str {
    let mut parts = specifier.splitn(3, '/');
    let first = parts.next().unwrap_or(specifier);
    match (first.starts_with('@'), parts.next()) {
        (true, Some(second)) => &specifier[..first.len() + 1 + second.len()],
        _ => first,
    }
}

/// The package containing `file`: the one with the longest directory prefix.
fn package_of<'a>(packages: &'a [Package], file: &str) -> Option<&'a Package> {
    packages
        .iter()
        .filter(|p| p.dir == "." || file.starts_with(&format!("{}/", p.dir)))
        .max_by_key(|p| if p.dir == "." { 0 } else { p.dir.len() })
}

/// The directory of a file path (empty at the root).
fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// The directory above `dir`, or `None` at the root.
fn parent_dir(dir: &str) -> Option<String> {
    if dir.is_empty() || dir == "." {
        None
    } else {
        Some(parent(dir).to_owned())
    }
}

/// `a/b/../c` as `a/c`.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// JSON with comments and trailing commas (tsconfig's dialect) as plain JSON.
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(c);
            }
            ('/', Some('/')) => while chars.next_if(|&n| n != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                let mut previous = ' ';
                for n in chars.by_ref() {
                    if previous == '*' && n == '/' {
                        break;
                    }
                    previous = n;
                }
            }
            _ => out.push(c),
        }
    }
    remove_trailing_commas(&out)
}

/// Drops commas followed (after whitespace) by `}` or `]`, outside strings.
fn remove_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.get(index + 1) {
                    out.push(*next);
                }
                index += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == ',' {
            let next = chars[index + 1..].iter().find(|n| !n.is_whitespace());
            if !matches!(next, Some('}' | ']')) {
                out.push(c);
            }
        } else {
            in_string = c == '"';
            out.push(c);
        }
        index += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{builtin, match_pattern, normalize, package_name, strip_jsonc};

    #[test]
    fn specifiers_name_their_package_or_builtin() {
        assert_eq!(package_name("lodash/fp"), "lodash");
        assert_eq!(package_name("@acme/ui/button"), "@acme/ui");
        assert_eq!(package_name("@acme"), "@acme");
        assert_eq!(builtin("node:fs/promises"), Some("fs"));
        assert_eq!(builtin("path"), Some("path"));
        assert_eq!(builtin("node:test"), Some("test"));
        assert_eq!(builtin("test"), None);
    }

    #[test]
    fn patterns_capture_what_their_star_stands_for() {
        assert_eq!(match_pattern("@app/*", "@app/a/b").as_deref(), Some("a/b"));
        assert_eq!(match_pattern("./*.js", "./x.js").as_deref(), Some("x"));
        assert_eq!(match_pattern("exact", "exact").as_deref(), Some(""));
        assert_eq!(match_pattern("@app/*", "@other/a"), None);
    }

    #[test]
    fn tsconfig_comments_and_trailing_commas_are_removed_outside_strings() {
        let text = "{\n  // note\n  \"a\": \"http://x\", /* b */\n  \"c\": [1, 2,],\n}\n";
        let value: serde_json::Value = serde_json::from_str(&strip_jsonc(text)).unwrap();
        assert_eq!(value, serde_json::json!({ "a": "http://x", "c": [1, 2] }));
    }

    #[test]
    fn paths_normalize_parent_segments() {
        assert_eq!(normalize("src/a/../b"), "src/b");
        assert_eq!(normalize("./a/./b"), "a/b");
    }
}
