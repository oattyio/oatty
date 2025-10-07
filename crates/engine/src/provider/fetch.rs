use anyhow::anyhow;
use heroku_registry::CommandSpec;
use heroku_util::http_path_resolution::build_path;
use serde_json::{Map as JsonMap, Value};

pub trait ProviderValueFetcher: Send + Sync {
    fn fetch_list(&self, spec: CommandSpec, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>>;
}

pub struct DefaultHttpFetcher;

impl ProviderValueFetcher for DefaultHttpFetcher {
    fn fetch_list(&self, mut spec: CommandSpec, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>> {
        // Resolve path placeholders from args when present
        let spec_name = spec.name.clone();
        let http = match spec.http_mut() {
            Some(http) => http,
            None => return Err(anyhow!("provider command '{}' is not HTTP-backed", spec_name)),
        };
        if !args.is_empty() {
            let updated_path = build_path(&http.path, args);
            http.path = updated_path;
        }
        let res = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(async move { heroku_util::http_exec::fetch_json_array(&spec).await }),
            Err(e) => Err(format!("runtime init failed: {}", e)),
        };
        res.map_err(anyhow::Error::msg)
    }
}
