// ABOUTME: Entry points for the differential engine (ADR-093 §4): `[lang.<id>] differential_entries` parsed as
// ABOUTME: `<file>:<function>` and resolved to the function's symbol and public signature.
use cupel_core::syntax::{ApiItemKind, ApiSurface, SymbolDef, SymbolKey, SymbolKind, Symbols};
use cupel_core::{Error, Result};
use serde_json::{Map, Value};

/// The `[lang.<id>]` key that lists a language's entry points.
pub const ENTRIES_KEY: &str = "differential_entries";

/// One configured entry point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// As configured: `<file>:<function>`.
    pub text: String,
    /// Relative to the repository root.
    pub file: String,
    pub function: String,
}

/// An entry's function as the language facts describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub symbol: SymbolKey,
    /// The enclosing namespace (`Symbols`), such as a Python module's dotted path.
    pub namespace: String,
    /// The normalized signature (`ApiSurface`).
    pub signature: String,
}

/// The entry points a language's `[lang.<id>]` table lists; none when it lists none.
pub fn configured(config: &Map<String, Value>) -> Result<Vec<Entry>> {
    let Some(value) = config.get(ENTRIES_KEY) else {
        return Ok(Vec::new());
    };
    let invalid = || Error::InvalidValue {
        what: ENTRIES_KEY,
        value: value.to_string(),
        expected: "a list of \"<file>:<function>\" strings",
    };
    value
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|item| item.as_str().and_then(parse).ok_or_else(invalid))
        .collect()
}

/// `<file>:<function>`, split at the last colon.
pub fn parse(text: &str) -> Option<Entry> {
    let (file, function) = text.rsplit_once(':')?;
    let valid = |part: &str| !part.trim().is_empty();
    (valid(file) && valid(function)).then(|| Entry {
        text: text.to_owned(),
        file: file.trim().to_owned(),
        function: function.trim().to_owned(),
    })
}

/// The function `entry` names in its file, with its public signature; why not, otherwise.
pub fn resolve(
    entry: &Entry,
    symbols: &Symbols,
    api: &ApiSurface,
) -> std::result::Result<Resolved, String> {
    let symbol = symbols
        .symbols
        .iter()
        .find(|symbol| {
            symbol.file == entry.file
                && symbol.name == entry.function
                && symbol.kind == SymbolKind::Function
        })
        .ok_or_else(|| format!("no function `{}` in `{}`", entry.function, entry.file))?;
    let item = api
        .items
        .iter()
        .find(|item| {
            item.file == entry.file
                && item.kind == ApiItemKind::Function
                && last_segment(&item.path) == entry.function
                && within(symbol, item.span.line)
        })
        .ok_or_else(|| {
            format!(
                "`{}` is not public: the harness calls what a module exports",
                entry.function
            )
        })?;
    Ok(Resolved {
        symbol: symbol.key(),
        namespace: symbol.namespace.clone(),
        signature: item.signature.clone(),
    })
}

fn within(symbol: &SymbolDef, line: u32) -> bool {
    (symbol.span.line..=symbol.span.end_line).contains(&line)
}

/// The last segment of a dotted or `::` path.
fn last_segment(path: &str) -> &str {
    let after_colons = path.rsplit("::").next().unwrap_or(path);
    after_colons.rsplit('.').next().unwrap_or(after_colons)
}

#[cfg(test)]
mod tests {
    use cupel_core::LangId;
    use cupel_core::syntax::{ApiItem, Span, Visibility};
    use serde_json::json;

    use super::*;

    fn span(line: u32, end_line: u32) -> Span {
        Span {
            line,
            col: 1,
            end_line,
            end_col: 1,
        }
    }

    fn symbol(name: &str, kind: SymbolKind) -> SymbolDef {
        SymbolDef {
            name: name.to_owned(),
            namespace: "pricing".to_owned(),
            display: format!("pricing.{name}"),
            kind,
            visibility: Visibility::Public,
            lang: LangId::from("python"),
            file: "src/pricing.py".to_owned(),
            span: span(3, 5),
            body: None,
            parameters: 1,
        }
    }

    fn item(path: &str, line: u32) -> ApiItem {
        ApiItem {
            path: path.to_owned(),
            kind: ApiItemKind::Function,
            signature: "(_: int) -> int".to_owned(),
            file: "src/pricing.py".to_owned(),
            span: span(line, line + 2),
        }
    }

    #[test]
    fn entries_are_a_list_of_file_colon_function() {
        let config = |value: Value| json!({ ENTRIES_KEY: value }).as_object().unwrap().clone();
        assert_eq!(
            configured(&config(json!(["src/a.py:f", "C:/x.ts:g"]))).unwrap(),
            vec![parse("src/a.py:f").unwrap(), parse("C:/x.ts:g").unwrap()]
        );
        assert_eq!(parse("C:/x.ts:g").unwrap().file, "C:/x.ts");
        assert!(configured(&Map::new()).unwrap().is_empty());
        assert!(configured(&config(json!("src/a.py:f"))).is_err());
        assert!(configured(&config(json!(["src/a.py"]))).is_err());
    }

    #[test]
    fn an_entry_resolves_to_its_function_and_public_signature() {
        let entry = parse("src/pricing.py:quarter").unwrap();
        let symbols = Symbols {
            symbols: vec![symbol("quarter", SymbolKind::Function)],
        };
        let api = ApiSurface {
            items: vec![item("pricing.quarter", 3)],
        };
        let resolved = resolve(&entry, &symbols, &api).unwrap();
        assert_eq!(
            (resolved.symbol.line, resolved.namespace, resolved.signature),
            (3, "pricing".to_owned(), "(_: int) -> int".to_owned())
        );
    }

    #[test]
    fn a_missing_method_or_private_function_is_not_compared() {
        let entry = parse("src/pricing.py:quarter").unwrap();
        let method = Symbols {
            symbols: vec![symbol("quarter", SymbolKind::Method)],
        };
        let function = Symbols {
            symbols: vec![symbol("quarter", SymbolKind::Function)],
        };
        let elsewhere = ApiSurface {
            items: vec![item("pricing.quarter", 9)],
        };
        assert_eq!(
            resolve(&entry, &method, &ApiSurface::default()),
            Err("no function `quarter` in `src/pricing.py`".to_owned())
        );
        assert_eq!(
            resolve(&entry, &function, &elsewhere),
            Err("`quarter` is not public: the harness calls what a module exports".to_owned())
        );
    }
}
