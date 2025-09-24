use serde_json::Value;

use super::contract::ProviderContract;

pub struct NullProvider;

impl crate::provider::ProviderRegistry for NullProvider {
    fn fetch_values(&self, _provider_id: &str, _arguments: &serde_json::Map<String, Value>) -> anyhow::Result<Vec<Value>> {
        Ok(Vec::new())
    }

    fn get_contract(&self, _provider_id: &str) -> Option<ProviderContract> {
        None
    }
}
