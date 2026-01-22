use anyhow::anyhow;
use indexmap::IndexSet;
use oatty_registry::CommandSpec;
use oatty_types::{EnvVar, ExecOutcome};
use oatty_util::{block_on_future, exec_remote_for_provider};
use serde_json::{Map as JsonMap, Value};

pub trait ProviderValueFetcher: Send + Sync {
    fn fetch_list(
        &self,
        spec: CommandSpec,
        args: &JsonMap<String, Value>,
        base_url: &str,
        headers: &IndexSet<EnvVar>,
    ) -> anyhow::Result<Vec<Value>>;
}

pub struct DefaultHttpFetcher;

impl ProviderValueFetcher for DefaultHttpFetcher {
    fn fetch_list(
        &self,
        spec: CommandSpec,
        args: &JsonMap<String, Value>,
        base_url: &str,
        headers: &IndexSet<EnvVar>,
    ) -> anyhow::Result<Vec<Value>> {
        let spec_name = spec.name.clone();
        if spec.http().is_none() {
            return Err(anyhow!("provider command '{}' is not HTTP-backed", spec_name));
        }

        let body = args.clone();
        let base_url = base_url.to_string();
        let headers = headers.clone();
        let outcome = block_on_future(async move {
            exec_remote_for_provider(&spec, &base_url, &headers, body, 0)
                .await
                .map_err(anyhow::Error::msg)
        });

        match outcome {
            Ok(ExecOutcome::Http { payload, .. }) => match payload {
                Value::Array(items) => Ok(items),
                _ => Err(anyhow!("provider command '{}' returned non-array payload", spec_name)),
            },
            Ok(_) => Err(anyhow!("provider command '{}' returned non-http outcome", spec_name)),
            Err(error) => Err(anyhow!(error)),
        }
    }
}
