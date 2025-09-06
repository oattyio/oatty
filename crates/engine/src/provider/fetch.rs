use heroku_registry::CommandSpec;
use heroku_util::http_path_resolution::build_path;
use serde_json::{Map as JsonMap, Value};

pub trait ProviderValueFetcher: Send + Sync {
    fn fetch_list(&self, spec: &CommandSpec, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>>;
}

pub struct DefaultHttpFetcher;

impl ProviderValueFetcher for DefaultHttpFetcher {
    fn fetch_list(&self, spec: &CommandSpec, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>> {
        // Resolve path placeholders from args when present
        let mut resolved = spec.clone();
        if !args.is_empty() {
            resolved.path = build_path(&spec.path, args);
        }
        let res = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(async move { heroku_util::http_exec::fetch_json_array(&resolved).await }),
            Err(e) => Err(format!("runtime init failed: {}", e)),
        };
        res.map_err(|e| anyhow::anyhow!(e))
    }
}
