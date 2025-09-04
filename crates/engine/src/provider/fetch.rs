use serde_json::{Map as JsonMap, Value};

pub trait ProviderValueFetcher: Send + Sync {
    fn fetch_list(&self, path: &str, args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>>;
}

pub struct DefaultHttpFetcher;

impl ProviderValueFetcher for DefaultHttpFetcher {
    fn fetch_list(&self, path: &str, _args: &JsonMap<String, Value>) -> anyhow::Result<Vec<Value>> {
        let res = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(async move { heroku_util::http_exec::fetch_json_array(path).await }),
            Err(e) => Err(format!("runtime init failed: {}", e)),
        };
        res.map_err(|e| anyhow::anyhow!(e))
    }
}
