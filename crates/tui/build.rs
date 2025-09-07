//! # Builder for extracting date-like fields from the heroku schema
//!
//! This build script extracts date-like fields to include
//! in sources for date formatting.
use std::{collections::BTreeSet, env, fs, path::PathBuf};

fn main() {
    // Path to the repo root schemas directory from crates/tui
    let schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../schemas/heroku-schema.enhanced.json");

    // Re-run build script if schema changes
    println!("cargo:rerun-if-changed={}", schema_path.display());

    let data = match fs::read_to_string(&schema_path) {
        Ok(s) => s,
        Err(_) => {
            // If schema missing in this environment, still generate an empty list
            write_output(&[]);
            return;
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => {
            write_output(&[]);
            return;
        }
    };

    let mut keys = BTreeSet::new();
    collect_date_like_keys(&value, &mut keys);

    let mut list: Vec<String> = keys.into_iter().collect();
    list.sort();
    write_output(&list);
}

fn write_output(keys: &[String]) {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = PathBuf::from(out_dir).join("date_fields.rs");
    let mut body = String::from("pub const DATE_FIELD_KEYS: &[&str] = &[\n");
    for k in keys {
        body.push_str("    \"");
        body.push_str(k);
        body.push_str("\",\n");
    }
    body.push_str("];");
    fs::write(dest_path, body).expect("failed to write generated date_fields.rs");
}

fn collect_date_like_keys(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    use serde_json::Value::*;
    match v {
        Object(map) => {
            // If this object itself is a field schema, check its format/example
            if is_date_schema_obj(map)
                && let Some(name) = map.get("title").and_then(|t| t.as_str())
            {
                // Titles are not reliable field names; ignore.
                let _ = name; // placeholder to avoid unused warnings
            }

            // Look for properties/definitions
            if let Some(Object(props)) = map.get("properties") {
                for (k, schema) in props {
                    if has_date_indicator(schema) {
                        out.insert(k.to_ascii_lowercase());
                    }
                    collect_date_like_keys(schema, out);
                }
            }
            if let Some(Object(defs)) = map.get("definitions") {
                for (k, schema) in defs {
                    if has_date_indicator(schema) {
                        out.insert(k.to_ascii_lowercase());
                    }
                    collect_date_like_keys(schema, out);
                }
            }

            // Recurse common schema containers
            let keys = [
                "items",
                "anyOf",
                "oneOf",
                "allOf",
                "not",
                "additionalProperties",
                "patternProperties",
                // nested structures
                "targetSchema",
                "schema",
                "properties",
                "definitions",
            ];
            for key in keys {
                if let Some(val) = map.get(key) {
                    collect_date_like_keys(val, out);
                }
            }
        }
        Array(arr) => {
            for item in arr {
                collect_date_like_keys(item, out);
            }
        }
        _ => {}
    }
}

fn has_date_indicator(v: &serde_json::Value) -> bool {
    use serde_json::Value::*;
    match v {
        Object(map) => {
            // Check explicit format markers
            let fmt_is_date = map
                .get("format")
                .and_then(|f| f.as_str())
                .map(|s| matches!(s, "date-time" | "date"))
                .unwrap_or(false);
            if fmt_is_date {
                return true;
            }

            // Examples look like ISO8601
            if let Some(example) = map.get("example").and_then(|e| e.as_str())
                && looks_like_iso_date(example)
            {
                return true;
            }

            // Otherwise, dig into nested composition
            if let Some(Array(arr)) = map.get("anyOf") {
                return arr.iter().any(has_date_indicator);
            }
            if let Some(Array(arr)) = map.get("oneOf") {
                return arr.iter().any(has_date_indicator);
            }
            if let Some(Array(arr)) = map.get("allOf") {
                return arr.iter().any(has_date_indicator);
            }
            if let Some(val) = map.get("items") {
                return has_date_indicator(val);
            }
            false
        }
        _ => false,
    }
}

fn is_date_schema_obj(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    map.get("format")
        .and_then(|f| f.as_str())
        .map(|s| matches!(s, "date-time" | "date"))
        .unwrap_or(false)
}

fn looks_like_iso_date(s: &str) -> bool {
    // Simple check for YYYY-MM-DD with optional time suffix
    if s.len() < 10 {
        return false;
    }
    let b = s.as_bytes();

    b.get(0..4)
        .map(|r| r.iter().all(|c| c.is_ascii_digit()))
        .unwrap_or(false)
        && matches!(b.get(4), Some(b'-' | b'/'))
        && b.get(5..7)
            .map(|r| r.iter().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
        && matches!(b.get(7), Some(b'-' | b'/'))
        && b.get(8..10)
            .map(|r| r.iter().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
}
