use ratatui::{prelude::*, widgets::*};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

use crate::theme;

// Lightweight table model + renderer for JSON arrays.

pub fn draw_json_table(f: &mut Frame, area: Rect, json: &Value) {
    draw_json_table_with_offset(f, area, json, 0);
}

pub fn draw_json_table_with_offset(f: &mut Frame, area: Rect, json: &Value, offset: usize) {
    // Find the array to render: either the value itself, or the first array field of an object
    let arr = match json {
        Value::Array(a) => Some(a.as_slice()),
        Value::Object(m) => m.values().find_map(|v| match v {
            Value::Array(a) => Some(a.as_slice()),
            _ => None,
        }),
        _ => None,
    };
    if arr.is_none() {
        let p = Paragraph::new("No tabular data in JSON").style(theme::text_muted());
        f.render_widget(p, area);
        return;
    }
    let arr = arr.unwrap();
    if arr.is_empty() {
        let p = Paragraph::new("No rows").style(theme::text_muted());
        f.render_widget(p, area);
        return;
    }

    let columns = infer_columns(arr);
    let headers: Vec<_> = columns
        .iter()
        .map(|k| Cell::from(normalize_header(k)).style(theme::title_style()))
        .collect();

    // Build rows
    let mut rows: Vec<Row> = Vec::new();
    for item in arr.iter() {
        let mut cells: Vec<Cell> = Vec::new();
        for key in &columns {
            let val = item.get(key).unwrap_or(&Value::Null);
            let txt = render_value(key, val);
            cells.push(Cell::from(txt).style(theme::text_style()));
        }
        rows.push(Row::new(cells));
    }

    // Column widths: split area width evenly with a floor
    let col_count = columns.len() as u16;
    let mut widths: Vec<Constraint> = Vec::new();
    if col_count == 0 {
        widths.push(Constraint::Percentage(100));
    } else {
        let per = (100 / col_count.max(1)).max(1);
        for _ in 0..col_count {
            widths.push(Constraint::Percentage(per));
        }
    }

    // Determine visible height to slice rows for scrolling (account for borders + header)
    let inner_height = area.height.saturating_sub(2); // block borders
    let header_rows = 1u16;
    let visible = inner_height.saturating_sub(header_rows).max(1) as usize;
    let start = offset.min(rows.len().saturating_sub(1));
    let end = (start + visible).min(rows.len());
    let rows_slice = if start < end {
        rows[start..end].to_vec()
    } else {
        Vec::new()
    };

    let table = Table::new(rows_slice, widths)
        .header(Row::new(headers))
        .block(
            Block::default()
                .title(Span::styled("Results", theme::title_style()))
                .borders(Borders::ALL)
                .border_style(theme::border_style(false)),
        )
        .column_spacing(1)
        .row_highlight_style(theme::list_highlight_style());

    f.render_widget(table, area);
}

// Choose columns using schema-informed + heuristic scoring and ensure at least 4 columns.
fn infer_columns(arr: &[Value]) -> Vec<String> {
    let mut score: HashMap<String, i32> = HashMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let sample = arr.iter().take(50); // sample up to 50 rows
    for item in sample {
        if let Value::Object(map) = item {
            for (k, v) in map.iter() {
                seen.insert(k.clone());
                let mut s = base_key_score(k) + property_frequency_boost(k);
                // Penalize nested arrays/objects (not scalar-ish)
                match v {
                    Value::Array(a) => s -= (a.len() as i32).min(3) + 3,
                    Value::Object(_) => s -= 5,
                    Value::String(sv) if sv.len() > 80 => s -= 3,
                    _ => {}
                }
                *score.entry(k.clone()).or_insert(0) += s;
            }
        }
    }
    let mut keys: Vec<(String, i32)> = seen
        .into_iter()
        .map(|k| (k.clone(), *score.get(&k).unwrap_or(&0)))
        .collect();
    keys.sort_by(|a, b| b.1.cmp(&a.1));
    let mut cols: Vec<String> = keys.into_iter().take(6).map(|(k, _)| k).collect();
    if cols.len() < 4 {
        // Ensure at least 4 columns by adding additional keys by frequency of appearance
        let mut freq: HashMap<String, usize> = HashMap::new();
        for item in arr.iter().take(100) {
            if let Value::Object(map) = item {
                for k in map.keys() {
                    *freq.entry(k.clone()).or_insert(0) += 1;
                }
            }
        }
        let mut extras: Vec<(String, usize)> = freq
            .into_iter()
            .filter(|(k, _)| !cols.contains(k))
            .collect();
        extras.sort_by(|a, b| b.1.cmp(&a.1));
        for (k, _) in extras.into_iter() {
            cols.push(k);
            if cols.len() >= 4 {
                break;
            }
        }
    }
    cols
}

fn base_key_score(k: &str) -> i32 {
    let l = k.to_lowercase();
    let mut s = 0;
    if l == "id" || l.ends_with("_id") {
        s += 10;
    }
    if l.contains("name") {
        s += 9;
    }
    if l.contains("status") || l.contains("state") {
        s += 8;
    }
    if l.contains("created_at") || l.contains("updated_at") || l.ends_with("_at") {
        s += 7;
    }
    if l.contains("owner") || l.contains("app") {
        s += 4;
    }
    s
}

// Frequency-based boost derived from schemas/top_properties.py output (top common payload keys)
fn property_frequency_boost(k: &str) -> i32 {
    let l = k.to_lowercase();
    match l.as_str() {
        // Very common, highly informative
        "id" => 12,
        "name" => 11,
        // Timestamps
        "created_at" | "updated_at" => 8,
        // Common resource scoping/identity
        "app" | "owner" | "email" => 6,
        // Lifecycle/status
        "type" | "state" | "status" => 6,
        // Misc descriptive
        "description" => 3,
        // Resource context
        "region" | "team" | "stack" | "user" | "plan" | "pipeline" => 5,
        // URLs
        "url" | "web_url" | "git_url" => 4,
        // roles and others
        "role" => 3,
        _ => 0,
    }
}

fn normalize_header(k: &str) -> String {
    let s = k.to_string();
    if s.ends_with("_id") {
        return s.trim_end_matches("_id").to_string() + " ID";
    }
    if s.ends_with("_url") {
        return s.trim_end_matches("_url").to_string() + " URL";
    }
    // snake_case to Title Case, preserve common acronyms
    let parts: Vec<String> = s.split('_').map(|p| preserve_acronym(p)).collect();
    parts.join(" ")
}

fn preserve_acronym(p: &str) -> String {
    match p.to_ascii_uppercase().as_str() {
        "ID" => "ID".into(),
        "URL" => "URL".into(),
        "HTTP" => "HTTP".into(),
        _ => capitalize(p),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn render_value(key: &str, v: &Value) -> String {
    // Redact when key looks sensitive
    let sensitive = is_sensitive_key(key);
    let raw = match v {
        Value::Null => "".to_string(),
        Value::Bool(b) => if *b { "✓" } else { "✗" }.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.len()),
        Value::Object(_) => "{…}".to_string(),
    };
    let out = if sensitive { mask_secret(&raw) } else { raw };
    ellipsize_middle_if_sha_like(&out, 16)
}

fn is_sensitive_key(k: &str) -> bool {
    let l = k.to_lowercase();
    l.contains("token") || l.contains("password") || l.contains("api_key") || l.contains("secret")
}

fn mask_secret(_s: &str) -> String {
    "••••".into()
}

fn ellipsize_middle_if_sha_like(s: &str, keep_total: usize) -> String {
    // Heuristic: hex-looking and long → compress
    let is_hexish = s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit());
    if !is_hexish || s.len() <= keep_total {
        return s.to_string();
    }
    let head = keep_total / 2;
    let tail = keep_total - head;
    format!("{}…{}", &s[..head], &s[s.len() - tail..])
}

// Optional sample data for debug/demo usage
#[allow(dead_code)]
pub fn sample_apps() -> Value {
    serde_json::json!([
        {"id":"1f2e3d4c5b6a7", "name":"my-app", "owner":"me@example.com", "region":"us", "created_at":"2024-08-01T12:00:00Z", "status":"succeeded"},
        {"id":"abcdeffedcba1234", "name":"api", "owner":"me@example.com", "region":"eu", "created_at":"2024-08-02T08:10:00Z", "status":"running"}
    ])
}

// Render a single-object result as key: value list, or a scalar as-is.
pub fn draw_kv_or_text(f: &mut Frame, area: Rect, json: &Value) {
    match json {
        Value::Object(map) => {
            // Sort keys using the same scoring
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort_by(|a, b| {
                let sa = base_key_score(a) + property_frequency_boost(a);
                let sb = base_key_score(b) + property_frequency_boost(b);
                sb.cmp(&sa)
            });
            let mut lines: Vec<Line> = Vec::new();
            for k in keys.iter().take(24) {
                let val = render_value(k, map.get(k).unwrap_or(&Value::Null));
                lines.push(Line::from(vec![
                    Span::styled(normalize_header(k), theme::title_style()),
                    Span::raw(": "),
                    Span::styled(val, theme::text_style()),
                ]));
            }
            let p = Paragraph::new(Text::from(lines))
                .block(
                    Block::default()
                        .title(Span::styled("Details", theme::title_style()))
                        .borders(Borders::ALL)
                        .border_style(theme::border_style(false))
                        .style(Style::default().bg(theme::BG_PANEL)),
                )
                .wrap(Wrap { trim: false })
                .style(theme::text_style());
            f.render_widget(p, area);
        }
        other => {
            let s = match other {
                Value::String(s) => s.clone(),
                _ => other.to_string(),
            };
            let p = Paragraph::new(s)
                .block(
                    Block::default()
                        .title(Span::styled("Result", theme::title_style()))
                        .borders(Borders::ALL)
                        .border_style(theme::border_style(false))
                        .style(Style::default().bg(theme::BG_PANEL)),
                )
                .wrap(Wrap { trim: false })
                .style(theme::text_style());
            f.render_widget(p, area);
        }
    }
}
