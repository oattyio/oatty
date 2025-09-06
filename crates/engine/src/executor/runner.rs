use anyhow::Result;
use reqwest::Method;
use serde_json::Value;
use std::str::FromStr;

use crate::resolve::RunContext;

use heroku_api::HerokuClient;
use heroku_registry::{CommandSpec, Registry};
use heroku_util::{
    build_path,
    http::{build_range_header_from_body, strip_range_body_fields},
};

/// Execute a single command.
///
/// Engines can provide concrete implementations that call HTTP, CLI, or other backends.
/// The default runner is a no-op façade that echoes inputs for testing and previews.
pub trait CommandRunner {
    /// Execute the given `run` command with optional named `with` parameters and JSON `body`.
    ///
    /// Implementations may use the `ctx` for read-only access to inputs, env, or previous
    /// step outputs to influence execution.
    fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, ctx: &RunContext) -> Result<Value>;
}

/// A simple runner that returns a synthetic JSON payload. This allows tests and
/// previews without external side effects.
pub struct NoopRunner;
impl CommandRunner for NoopRunner {
    fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, _ctx: &RunContext) -> Result<Value> {
        let mut obj = serde_json::Map::new();
        obj.insert("run".into(), Value::String(run.to_string()));
        if let Some(w) = with {
            obj.insert("with".into(), w.clone());
        }
        if let Some(b) = body {
            obj.insert("body".into(), b.clone());
        }
        Ok(Value::Object(obj))
    }
}

/// Registry-backed command runner that resolves `run` identifiers via the
/// command registry and executes HTTP requests with the Heroku API client.
pub struct RegistryCommandRunner {
    registry: Registry,
    client: HerokuClient,
}

impl RegistryCommandRunner {
    /// Create a new registry-backed runner from explicit dependencies.
    pub fn new(registry: Registry, client: HerokuClient) -> Self {
        Self { registry, client }
    }

    /// Create a new registry-backed runner by loading the embedded schema and
    /// constructing a `HerokuClient` from environment variables.
    pub fn from_spec(spec: &CommandSpec) -> Result<Self> {
        let registry = Registry::from_embedded_schema()?;
        let client = HerokuClient::new_from_service_id(spec.service_id)?;
        Ok(Self { registry, client })
    }
}

impl CommandRunner for RegistryCommandRunner {
    fn run(&self, run: &str, with: Option<&Value>, body: Option<&Value>, _ctx: &RunContext) -> Result<Value> {
        // Parse run into group + name (name may contain additional colons)
        let (group, name) = run
            .split_once(':')
            .map(|(g, rest)| (g.to_string(), rest.to_string()))
            .ok_or_else(|| anyhow::anyhow!("invalid run identifier: {}", run))?;

        let spec = self.registry.find_by_group_and_cmd(&group, &name)?;
        let method = Method::from_str(&spec.method).unwrap_or(Method::GET);

        // Inputs map from `with` if object
        let mut with_map: serde_json::Map<String, Value> = match with {
            Some(Value::Object(m)) => m.clone(),
            _ => serde_json::Map::new(),
        };

        // Build path variables from positional arg names, if present
        let mut path_variables = serde_json::Map::new();
        for pa in &spec.positional_args {
            if let Some(val) = with_map.remove(&pa.name) {
                path_variables.insert(pa.name.clone(), val);
            }
        }

        let path = build_path(&spec.path, &path_variables);
        let mut req = self.client.request(method.clone(), &path);

        match method {
            Method::GET | Method::DELETE => {
                if !with_map.is_empty() {
                    // Convert remaining entries to query params
                    let query: Vec<(String, String)> = with_map
                        .into_iter()
                        .map(|(k, v)| {
                            let s = match v {
                                Value::String(s) => s,
                                other => other.to_string(),
                            };
                            (k, s)
                        })
                        .collect();
                    req = req.query(&query);
                }
            }
            _ => {
                // Prefer body if provided; otherwise, fall back to remaining `with` map as body
                let mut body_obj: serde_json::Map<String, Value> = match body {
                    Some(Value::Object(m)) => m.clone(),
                    Some(other) => serde_json::Map::from_iter([("value".into(), other.clone())]),
                    None => with_map,
                };

                // Build Range header if present and strip body fields
                if let Some(range_header) = build_range_header_from_body(&body_obj) {
                    req = req.header("Range", range_header);
                    body_obj = strip_range_body_fields(body_obj);
                }
                req = req.json(&Value::Object(body_obj));
            }
        }

        // Execute request synchronously using a lightweight runtime
        let res = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(async move {
                let resp = req.send().await.map_err(|e| anyhow::anyhow!(e))?;
                let status = resp.status();
                let headers = resp.headers().clone();
                let val = resp.json::<Value>().await.unwrap_or(Value::Null);
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "status_code".into(),
                    Value::Number(serde_json::Number::from(status.as_u16())),
                );
                if let Some(v) = headers
                    .get("Content-Range")
                    .and_then(|h| h.to_str().ok())
                    .map(|s| Value::String(s.to_string()))
                {
                    obj.insert("content_range".into(), v);
                }
                obj.insert("data".into(), val);
                Ok::<Value, anyhow::Error>(Value::Object(obj))
            }),
            Err(e) => Err(anyhow::anyhow!(e)),
        }?;

        Ok(res)
    }
}
