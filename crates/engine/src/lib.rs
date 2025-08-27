use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowFile {
    #[serde(default)]
    pub workflows: HashMap<String, Workflow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Workflow {
    #[serde(default)]
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub with: Value,
    #[serde(default)]
    pub r#if: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct ContextState {
    pub env: HashMap<String, String>,
    pub tasks: HashMap<String, TaskResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskResult {
    pub status: String,
    pub output: Value,
    #[serde(default)]
    pub logs: Vec<String>,
}

pub fn load_workflow_from_file(path: impl AsRef<Path>) -> Result<WorkflowFile> {
    let bytes = fs::read(path.as_ref()).with_context(|| format!("read {}", path.as_ref().display()))?;
    let s = String::from_utf8_lossy(&bytes);
    if path.as_ref().extension().and_then(|x| x.to_str()) == Some("json") {
        let wf: WorkflowFile = serde_json::from_str(&s).context("parse workflow json")?;
        Ok(wf)
    } else {
        let wf: WorkflowFile = serde_yaml::from_str(&s).context("parse workflow yaml")?;
        Ok(wf)
    }
}

pub fn interpolate_value(v: &Value, ctx: &ContextState) -> Value {
    match v {
        Value::String(s) => Value::String(interpolate_string(s, ctx)),
        Value::Array(arr) => Value::Array(arr.iter().map(|x| interpolate_value(x, ctx)).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map.iter() {
                out.insert(k.clone(), interpolate_value(val, ctx));
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

fn interpolate_string(s: &str, ctx: &ContextState) -> String {
    // Replace occurrences of ${{ path }} with looked-up values from ctx
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${{") {
        let (head, tail) = rest.split_at(start);
        out.push_str(head);
        if let Some(end_idx) = tail.find("}}") {
            let expr = &tail[3..end_idx].trim();
            let val = resolve_expr(expr, ctx).unwrap_or_default();
            out.push_str(&val);
            rest = &tail[end_idx + 2..];
        } else {
            // No closing, bail out
            out.push_str(tail);
            break;
        }
    }
    if out.is_empty() {
        s.to_string()
    } else {
        out.push_str(rest);
        out
    }
}

fn resolve_expr(expr: &str, ctx: &ContextState) -> Option<String> {
    // Support tasks.<name>.output.<path>, env.<VAR>, or simple equality in if (a == b)
    if let Some(eq_pos) = expr.find("==") {
        let left = expr[..eq_pos].trim();
        let right = expr[eq_pos + 2..].trim().trim_matches('"');
        let left_val = resolve_expr(left, ctx).unwrap_or_default();
        return Some(((left_val == right) as i32).to_string());
    }
    if let Some(stripped) = expr.strip_prefix("env.") {
        return ctx.env.get(stripped).cloned();
    }
    if let Some(rem) = expr.strip_prefix("tasks.") {
        // tasks.<name>.output.path.to.leaf
        let mut parts = rem.split('.');
        let task_name = parts.next()?;
        let rest: Vec<&str> = parts.collect();
        let output = &ctx.tasks.get(task_name)?.output;
        let mut cur = output;
        for p in rest {
            match cur {
                Value::Object(map) => {
                    cur = map.get(p)?;
                }
                _ => return None,
            }
        }
        return Some(match cur {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            other => other.to_string(),
        });
    }
    None
}
