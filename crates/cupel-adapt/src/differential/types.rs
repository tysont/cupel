// ABOUTME: Parameter types for the differential engine (ADR-093 §5): each language's normalized `ApiSurface`
// ABOUTME: signature mapped to one neutral model the generators and shrinkers understand.

/// What a generator can make for one parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// An integer within a range.
    Int {
        min: i64,
        max: i64,
    },
    /// A JavaScript number: mostly integers within ±2^53, some halves.
    Number,
    Float,
    Bool,
    Str,
    List(Box<Ty>),
    /// The type or null.
    Optional(Box<Ty>),
    /// An untyped parameter: integers, strings, booleans, null, lists of integers.
    Any,
}

/// One parameter: its neutral type and its type as the language spells it, which the Go and
/// Rust harnesses decode into.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub ty: Ty,
    pub text: String,
}

/// A function's parameters and how many results it returns (Go assigns each one).
#[derive(Debug, Clone, PartialEq)]
pub struct Signature {
    pub params: Vec<Param>,
    pub results: usize,
}

impl Signature {
    /// Parameters only, for languages whose harness does not count results.
    fn of(params: Vec<Param>) -> Self {
        Self { params, results: 1 }
    }
}

/// Why a signature has no generator: the parameter type, as written.
pub type Unsupported = String;

/// `text` split at each top-level `separator`, outside brackets, braces, and parentheses.
pub fn split_top(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0_i32, 0);
    for (at, c) in text.char_indices() {
        match c {
            '(' | '[' | '<' | '{' => depth += 1,
            ')' | ']' | '>' | '}' => depth -= 1,
            c if c == separator && depth == 0 => {
                parts.push(text[start..at].trim());
                start = at + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts.retain(|part| !part.is_empty());
    parts
}

/// The text inside the parentheses that open `signature`, and what follows them.
fn parenthesized(signature: &str) -> Option<(&str, &str)> {
    let body = signature.trim().strip_prefix('(')?;
    let mut depth = 1;
    for (at, c) in body.char_indices() {
        match c {
            '(' | '[' | '<' | '{' => depth += 1,
            ')' | ']' | '>' | '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some((&body[..at], body[at + 1..].trim()));
        }
    }
    None
}

/// The parameter list of `signature`, each with its `: type` annotation (or none).
fn annotated(signature: &str) -> Result<Vec<Option<&str>>, Unsupported> {
    let (params, _) = parenthesized(signature).ok_or_else(|| signature.to_owned())?;
    split_top(params, ',')
        .into_iter()
        .map(|param| {
            let param = split_top(param, '=')[0];
            if param.starts_with('*') || param.starts_with("...") {
                return Err(param.to_owned());
            }
            Ok(param
                .split_once(':')
                .map(|(_, ty)| ty.trim().trim_start_matches('?')))
        })
        .collect()
}

/// A Python signature, such as `(_: list[int], _) -> int`.
pub fn python(signature: &str) -> Result<Signature, Unsupported> {
    let params = annotated(signature)?
        .into_iter()
        .map(|ty| match ty {
            None => Ok(param(Ty::Any, "")),
            Some(text) => python_type(text).map(|ty| param(ty, text)),
        })
        .collect::<Result<_, _>>()?;
    Ok(Signature::of(params))
}

fn python_type(text: &str) -> Result<Ty, Unsupported> {
    let text = text.trim();
    let alternatives = split_top(text, '|');
    if alternatives.len() == 2 && alternatives.contains(&"None") {
        let other = alternatives
            .iter()
            .find(|alt| **alt != "None")
            .expect("two");
        return Ok(Ty::Optional(Box::new(python_type(other)?)));
    }
    if let Some(inner) = generic(text, &["Optional"]) {
        return Ok(Ty::Optional(Box::new(python_type(inner)?)));
    }
    if let Some(inner) = generic(text, &["list", "List", "Sequence"]) {
        return Ok(Ty::List(Box::new(python_type(inner)?)));
    }
    match text {
        "int" => Ok(int(i64::MIN, i64::MAX)),
        "float" => Ok(Ty::Float),
        "bool" => Ok(Ty::Bool),
        "str" => Ok(Ty::Str),
        "Any" | "object" => Ok(Ty::Any),
        _ => Err(text.to_owned()),
    }
}

/// A TypeScript signature, such as `(_: number[], _: string): boolean`; an untyped parameter is
/// `any`, so a JavaScript signature (`(_, _)`) parses the same way.
pub fn ecma(signature: &str) -> Result<Signature, Unsupported> {
    let params = annotated(signature)?
        .into_iter()
        .map(|ty| match ty {
            None => Ok(param(Ty::Any, "")),
            Some(text) => ecma_type(text).map(|ty| param(ty, text)),
        })
        .collect::<Result<_, _>>()?;
    Ok(Signature::of(params))
}

fn ecma_type(text: &str) -> Result<Ty, Unsupported> {
    let compact: String = text
        .trim()
        .trim_start_matches("readonly ")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let alternatives = split_top(&compact, '|');
    let nothing = |alt: &&str| *alt == "null" || *alt == "undefined";
    if alternatives.len() == 2 && alternatives.iter().any(nothing) {
        let other = alternatives.iter().find(|alt| !nothing(alt)).ok_or(text)?;
        return Ok(Ty::Optional(Box::new(ecma_type(other)?)));
    }
    if let Some(element) = compact.strip_suffix("[]") {
        return Ok(Ty::List(Box::new(ecma_type(element)?)));
    }
    if let Some(inner) = generic(&compact, &["Array", "ReadonlyArray"]) {
        return Ok(Ty::List(Box::new(ecma_type(inner)?)));
    }
    match compact.as_str() {
        "number" => Ok(Ty::Number),
        "string" => Ok(Ty::Str),
        "boolean" => Ok(Ty::Bool),
        "any" | "unknown" => Ok(Ty::Any),
        _ => Err(text.trim().to_owned()),
    }
}

/// A Go signature, such as `(_, _ string, _ []int64) (int64, error)`: names without a type take
/// the next type, and the results are counted.
pub fn go(signature: &str) -> Result<Signature, Unsupported> {
    let (params, results) = parenthesized(signature).ok_or_else(|| signature.to_owned())?;
    let mut typed: Vec<Param> = Vec::new();
    let mut pending = 0;
    for part in split_top(params, ',') {
        match part.split_once(' ') {
            None => pending += 1,
            Some((_, text)) => {
                let text = text.trim();
                let ty = go_type(text)?;
                typed.extend((0..=pending).map(|_| param(ty.clone(), text)));
                pending = 0;
            }
        }
    }
    if pending > 0 {
        return Err(params.to_owned());
    }
    Ok(Signature {
        params: typed,
        results: go_results(results),
    })
}

fn go_results(results: &str) -> usize {
    match parenthesized(results) {
        Some((inner, _)) => split_top(inner, ',').len(),
        None if results.is_empty() => 0,
        None => 1,
    }
}

fn go_type(text: &str) -> Result<Ty, Unsupported> {
    if let Some(element) = text.strip_prefix("[]") {
        return Ok(Ty::List(Box::new(go_type(element)?)));
    }
    sized(text, GO_INTEGERS)
        .or(match text {
            "float32" | "float64" => Some(Ty::Float),
            "bool" => Some(Ty::Bool),
            "string" => Some(Ty::Str),
            _ => None,
        })
        .ok_or_else(|| text.to_owned())
}

/// A Rust signature, such as `(_: &[i64], _: &str) -> Vec<String>`.
pub fn rust(signature: &str) -> Result<Signature, Unsupported> {
    let params = annotated(signature)?
        .into_iter()
        .map(|ty| {
            let text = ty.ok_or_else(|| signature.to_owned())?;
            rust_type(text).map(|ty| param(ty, text))
        })
        .collect::<Result<_, _>>()?;
    Ok(Signature::of(params))
}

/// What a Rust harness decodes a parameter into, and whether the call passes a reference to it:
/// `&str` decodes into `String`, `&[T]` into `Vec<T>`, `&T` into `T`.
pub fn rust_owned(text: &str) -> (String, bool) {
    let Some(borrowed) = text.trim().strip_prefix('&') else {
        return (text.trim().to_owned(), false);
    };
    let borrowed = without_lifetime(borrowed);
    let owned = match borrowed {
        "str" => "String".to_owned(),
        _ => match borrowed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            Some(element) => format!("Vec<{}>", element.trim()),
            None => borrowed.to_owned(),
        },
    };
    (owned, true)
}

fn without_lifetime(text: &str) -> &str {
    match text.trim().strip_prefix('\'') {
        Some(rest) => rest.split_once(' ').map_or(rest, |(_, ty)| ty).trim(),
        None => text.trim(),
    }
}

fn rust_type(text: &str) -> Result<Ty, Unsupported> {
    let text = text.trim();
    if text.starts_with("&mut") {
        return Err(text.to_owned());
    }
    let (owned, _) = rust_owned(text);
    if let Some(inner) = generic(&owned, &["Vec"]) {
        return Ok(Ty::List(Box::new(rust_type(inner)?)));
    }
    if let Some(inner) = generic(&owned, &["Option"]) {
        return Ok(Ty::Optional(Box::new(rust_type(inner)?)));
    }
    sized(&owned, RUST_INTEGERS)
        .or(match owned.as_str() {
            "f32" | "f64" => Some(Ty::Float),
            "bool" => Some(Ty::Bool),
            "String" => Some(Ty::Str),
            _ => None,
        })
        .ok_or_else(|| text.to_owned())
}

/// Integer types by name, with their ranges clamped to `i64`.
type Integers = [(&'static str, i64, i64)];

const GO_INTEGERS: &Integers = &[
    ("int", i64::MIN, i64::MAX),
    ("int8", i8::MIN as i64, i8::MAX as i64),
    ("int16", i16::MIN as i64, i16::MAX as i64),
    ("int32", i32::MIN as i64, i32::MAX as i64),
    ("rune", i32::MIN as i64, i32::MAX as i64),
    ("int64", i64::MIN, i64::MAX),
    ("uint", 0, i64::MAX),
    ("uint8", 0, u8::MAX as i64),
    ("byte", 0, u8::MAX as i64),
    ("uint16", 0, u16::MAX as i64),
    ("uint32", 0, u32::MAX as i64),
    ("uint64", 0, i64::MAX),
];

const RUST_INTEGERS: &Integers = &[
    ("i8", i8::MIN as i64, i8::MAX as i64),
    ("i16", i16::MIN as i64, i16::MAX as i64),
    ("i32", i32::MIN as i64, i32::MAX as i64),
    ("i64", i64::MIN, i64::MAX),
    ("isize", i64::MIN, i64::MAX),
    ("u8", 0, u8::MAX as i64),
    ("u16", 0, u16::MAX as i64),
    ("u32", 0, u32::MAX as i64),
    ("u64", 0, i64::MAX),
    ("usize", 0, i64::MAX),
];

/// The integer type `name` from `table`.
fn sized(name: &str, table: &Integers) -> Option<Ty> {
    table
        .iter()
        .find(|(integer, _, _)| *integer == name)
        .map(|(_, min, max)| int(*min, *max))
}

/// The argument of `text` when it is one of `names` applied to one type: `list[int]`,
/// `Vec<i64>`, `Array<number>`.
fn generic<'a>(text: &'a str, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| {
        let rest = text.strip_prefix(name)?;
        let inner = rest
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .or_else(|| {
                rest.strip_prefix('<')
                    .and_then(|rest| rest.strip_suffix('>'))
            })?;
        (split_top(inner, ',').len() == 1).then(|| inner.trim())
    })
}

fn int(min: i64, max: i64) -> Ty {
    Ty::Int { min, max }
}

fn param(ty: Ty, text: &str) -> Param {
    Param {
        ty,
        text: text.trim().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tys(signature: Result<Signature, Unsupported>) -> Vec<Ty> {
        signature
            .unwrap()
            .params
            .into_iter()
            .map(|param| param.ty)
            .collect()
    }

    fn list(ty: Ty) -> Ty {
        Ty::List(Box::new(ty))
    }

    const I64: Ty = Ty::Int {
        min: i64::MIN,
        max: i64::MAX,
    };

    #[test]
    fn python_annotations_map_to_types_and_unannotated_parameters_are_any() {
        assert_eq!(
            tys(python(
                "(_: int, _: list[str], _, _: Optional[bool]) -> int"
            )),
            vec![
                I64,
                list(Ty::Str),
                Ty::Any,
                Ty::Optional(Box::new(Ty::Bool))
            ]
        );
        assert_eq!(
            tys(python("(_: float | None = None)")),
            vec![Ty::Optional(Box::new(Ty::Float))]
        );
        assert_eq!(
            python("(_: dict[str, int])"),
            Err("dict[str, int]".to_owned())
        );
        assert_eq!(python("(*_)"), Err("*_".to_owned()));
    }

    #[test]
    fn typescript_types_map_and_javascript_parameters_are_any() {
        assert_eq!(
            tys(ecma(
                "(_: number[] [], _: readonly string[], _: boolean | null): number"
            )),
            vec![
                list(list(Ty::Number)),
                list(Ty::Str),
                Ty::Optional(Box::new(Ty::Bool))
            ]
        );
        assert_eq!(tys(ecma("(_, _)")), vec![Ty::Any, Ty::Any]);
        assert_eq!(ecma("(_: Invoice[])"), Err("Invoice".to_owned()));
    }

    #[test]
    fn go_names_without_a_type_take_the_next_type_and_results_are_counted() {
        let signature = go("(_, _ int8, _ []string) (int64, error)").unwrap();
        let int8 = Ty::Int {
            min: -128,
            max: 127,
        };
        assert_eq!(
            signature
                .params
                .iter()
                .map(|p| p.ty.clone())
                .collect::<Vec<_>>(),
            vec![int8.clone(), int8, list(Ty::Str)]
        );
        assert_eq!(signature.params[2].text, "[]string");
        assert_eq!(signature.results, 2);
        assert_eq!(go("(_ int64)").unwrap().results, 0);
        assert_eq!(go("(_ uint8) string").unwrap().results, 1);
        assert_eq!(go("(_ map[string]int)"), Err("map[string]int".to_owned()));
    }

    #[test]
    fn rust_borrowed_parameters_decode_into_owned_values() {
        assert_eq!(
            tys(rust(
                "(_: &[i64], _: &'a str, _: Option<u8>) -> Vec<String>"
            )),
            vec![
                list(I64),
                Ty::Str,
                Ty::Optional(Box::new(Ty::Int { min: 0, max: 255 }))
            ]
        );
        assert_eq!(
            rust_owned("&[Vec<i64>]"),
            ("Vec<Vec<i64>>".to_owned(), true)
        );
        assert_eq!(rust_owned("&'static str"), ("String".to_owned(), true));
        assert_eq!(rust_owned("u32"), ("u32".to_owned(), false));
        assert_eq!(rust("(_: &mut Vec<i64>)"), Err("&mut Vec<i64>".to_owned()));
        assert_eq!(rust("(_: char)"), Err("char".to_owned()));
        assert_eq!(rust("(&self) -> i64"), Err("(&self) -> i64".to_owned()));
    }

    #[test]
    fn splitting_respects_nesting() {
        assert_eq!(
            split_top("a, Map<b, c>, (d, e), [f, g]", ','),
            vec!["a", "Map<b, c>", "(d, e)", "[f, g]"]
        );
    }
}
