use heroku_registry::CommandSpec;
use serde_json::{Map as JsonMap, Value};

pub trait ProviderValueFetcher: Send + Sync {
    fn fetch_list(&self, spec: &CommandSpec, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>>;
}

pub struct DefaultHttpFetcher;

impl ProviderValueFetcher for DefaultHttpFetcher {
    fn fetch_list(&self, spec: &CommandSpec, _args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>> {
        let res = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(async move { heroku_util::http_exec::fetch_json_array(spec).await }),
            Err(e) => Err(format!("runtime init failed: {}", e)),
        };
        res.map_err(|e| anyhow::anyhow!(e))
    }
}
