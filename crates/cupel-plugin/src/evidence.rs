// ABOUTME: Validates a finding's evidence against the JSON Schema its check's manifest declares, using the small
// ABOUTME: keyword subset evidence needs and refusing every other keyword (ADR-043); the host and the matrix test share it.
use serde_json::{Map, Value};

/// Keywords that describe without constraining.
const ANNOTATIONS: [&str; 6] = [
    "$schema",
    "title",
    "description",
    "format",
    "default",
    "examples",
];

/// Checks `value` against `schema`. The error names the path to the first violation, or the
/// keyword this validator does not implement.
pub fn validate(schema: &Map<String, Value>, value: &Value) -> Result<(), String> {
    check(schema, value, "evidence")
}

/// Checks one keyword's `rule` against `value`, given the whole object `schema` it is in.
type Rule = fn(&Map<String, Value>, &Value, &Value, &str) -> Result<(), String>;

/// Every constraining keyword the validator implements, with its rule.
const RULES: [(&str, Rule); 9] = [
    ("type", |_, rule, value, at| check_type(rule, value, at)),
    ("properties", |_, rule, value, at| {
        check_properties(rule, value, at)
    }),
    ("required", |_, rule, value, at| {
        check_required(rule, value, at)
    }),
    ("additionalProperties", check_additional),
    ("items", |_, rule, value, at| check_items(rule, value, at)),
    ("enum", |_, rule, value, at| check_enum(rule, value, at)),
    ("const", |_, rule, value, at| check_const(rule, value, at)),
    ("minimum", |_, rule, value, at| {
        check_bound("minimum", rule, value, at)
    }),
    ("maximum", |_, rule, value, at| {
        check_bound("maximum", rule, value, at)
    }),
];

fn check(schema: &Map<String, Value>, value: &Value, at: &str) -> Result<(), String> {
    for (keyword, rule) in schema {
        if ANNOTATIONS.contains(&keyword.as_str()) {
            continue;
        }
        let (_, apply) = RULES
            .iter()
            .find(|(name, _)| name == keyword)
            .ok_or_else(|| {
                format!(
                    "{at}: the schema uses `{keyword}`, which the evidence validator does not \
                     implement (ADR-043)"
                )
            })?;
        apply(schema, rule, value, at)?;
    }
    Ok(())
}

fn check_enum(rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    let allowed = rule.as_array().ok_or_else(|| malformed("enum", at))?;
    if allowed.contains(value) {
        Ok(())
    } else {
        Err(format!("{at}: {value} is not one of {rule}"))
    }
}

fn check_const(rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    if rule == value {
        Ok(())
    } else {
        Err(format!("{at}: {value} is not {rule}"))
    }
}

fn malformed(keyword: &str, at: &str) -> String {
    format!("{at}: malformed `{keyword}` in the schema")
}

fn check_type(rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    let names: Vec<&str> = match rule {
        Value::String(name) => vec![name.as_str()],
        Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
        _ => return Err(malformed("type", at)),
    };
    if names.iter().any(|name| has_type(name, value)) {
        Ok(())
    } else {
        Err(format!("{at}: {value} is not of type {rule}"))
    }
}

fn has_type(name: &str, value: &Value) -> bool {
    match name {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        _ => false,
    }
}

fn check_properties(rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    let (Some(properties), Some(object)) = (rule.as_object(), value.as_object()) else {
        return Ok(());
    };
    for (name, field) in object {
        if let Some(Value::Object(schema)) = properties.get(name) {
            check(schema, field, &format!("{at}.{name}"))?;
        }
    }
    Ok(())
}

fn check_required(rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    let names = rule.as_array().ok_or_else(|| malformed("required", at))?;
    match names
        .iter()
        .filter_map(Value::as_str)
        .find(|name| !object.contains_key(*name))
    {
        Some(missing) => Err(format!("{at}: required property `{missing}` is missing")),
        None => Ok(()),
    }
}

fn check_additional(
    schema: &Map<String, Value>,
    rule: &Value,
    value: &Value,
    at: &str,
) -> Result<(), String> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    let declared = schema.get("properties").and_then(Value::as_object);
    for (name, field) in object {
        if declared.is_some_and(|properties| properties.contains_key(name)) {
            continue;
        }
        match rule {
            Value::Bool(true) => {}
            Value::Bool(false) => {
                return Err(format!("{at}: property `{name}` is not in the schema"));
            }
            Value::Object(extra) => check(extra, field, &format!("{at}.{name}"))?,
            _ => return Err(malformed("additionalProperties", at)),
        }
    }
    Ok(())
}

fn check_items(rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    let (Some(schema), Some(items)) = (rule.as_object(), value.as_array()) else {
        return Ok(());
    };
    items
        .iter()
        .enumerate()
        .try_for_each(|(index, item)| check(schema, item, &format!("{at}[{index}]")))
}

fn check_bound(keyword: &str, rule: &Value, value: &Value, at: &str) -> Result<(), String> {
    let (Some(bound), Some(number)) = (rule.as_f64(), value.as_f64()) else {
        return Ok(());
    };
    let within = if keyword == "minimum" {
        number >= bound
    } else {
        number <= bound
    };
    if within {
        Ok(())
    } else {
        Err(format!("{at}: {value} violates {keyword} {rule}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Map<String, Value> {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "Example",
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "count": {"type": "integer", "format": "uint32", "minimum": 0},
                "label": {"type": ["string", "null"], "enum": ["a", "b", null]},
                "sizes": {"type": "array", "items": {"type": "number", "maximum": 10}}
            },
            "required": ["count"]
        })
        .as_object()
        .cloned()
        .unwrap()
    }

    #[test]
    fn valid_evidence_passes() {
        let value = json!({"count": 3, "label": null, "sizes": [1.5, 10]});
        assert_eq!(validate(&schema(), &value), Ok(()));
    }

    #[test]
    fn each_violation_is_reported_with_its_path() {
        for (value, expected) in [
            (json!({"label": "a"}), "required property `count`"),
            (json!({"count": -1}), "evidence.count: -1 violates minimum"),
            (json!({"count": 1.5}), "evidence.count: 1.5 is not of type"),
            (
                json!({"count": 1, "label": "c"}),
                "evidence.label: \"c\" is not one of",
            ),
            (
                json!({"count": 1, "sizes": [11]}),
                "evidence.sizes[0]: 11 violates maximum",
            ),
            (
                json!({"count": 1, "extra": true}),
                "property `extra` is not in the schema",
            ),
            (json!([1]), "is not of type \"object\""),
        ] {
            let error = validate(&schema(), &value).unwrap_err();
            assert!(error.contains(expected), "{value}: {error}");
        }
    }

    #[test]
    fn an_unimplemented_keyword_fails_rather_than_passing_silently() {
        let schema = json!({"type": "object", "properties": {"a": {"$ref": "#/$defs/A"}}})
            .as_object()
            .cloned()
            .unwrap();
        let error = validate(&schema, &json!({"a": 1})).unwrap_err();
        assert!(error.contains("`$ref`"), "{error}");
    }
}
