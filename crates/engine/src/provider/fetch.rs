use anyhow::anyhow;
use indexmap::IndexSet;
use oatty_registry::CommandSpec;
use oatty_types::{EnvVar, ExecOutcome};
use oatty_util::{block_on_future, exec_remote_for_provider, http::extract_provider_collection_items};
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
        let list_response_path = spec.http().and_then(|http_spec| http_spec.list_response_path.clone());
        let base_url = base_url.to_string();
        let headers = headers.clone();
        let outcome = block_on_future(async move {
            exec_remote_for_provider(&spec, &base_url, &headers, body, 0)
                .await
                .map_err(anyhow::Error::msg)
        });

        match outcome {
            Ok(ExecOutcome::Http { payload, .. }) => extract_provider_items(payload, list_response_path.as_deref())
                .ok_or_else(|| anyhow!("provider command '{}' returned non-array payload", spec_name)),
            Ok(_) => Err(anyhow!("provider command '{}' returned non-http outcome", spec_name)),
            Err(error) => Err(anyhow!(error)),
        }
    }
}

fn extract_provider_items(payload: Value, list_response_path: Option<&str>) -> Option<Vec<Value>> {
    extract_provider_collection_items(&payload, list_response_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oatty_types::HttpCommandSpec;
    use serde_json::json;

    fn list_response_path_for_spec(command_spec: &CommandSpec) -> Option<&str> {
        command_spec.http().and_then(|http_spec| http_spec.list_response_path.as_deref())
    }

    fn build_provider_test_spec() -> CommandSpec {
        CommandSpec::new_http(
            "vendor".to_string(),
            "projects:list".to_string(),
            "List projects".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/projects", None, Some("projects".to_string())),
            1,
        )
    }

    #[test]
    fn extract_provider_items_returns_top_level_array() {
        let command_spec = build_provider_test_spec();
        let payload = json!([{ "id": 1 }, { "id": 2 }]);
        let items = extract_provider_items(payload, list_response_path_for_spec(&command_spec)).expect("array payload should be returned");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn extract_provider_items_unwraps_paginated_projects_wrapper() {
        let command_spec = build_provider_test_spec();
        let payload = json!({
            "pagination": { "count": 2, "next": null, "prev": 1770947393057u64 },
            "projects": [{ "id": "project-a" }, { "id": "project-b" }]
        });

        let items =
            extract_provider_items(payload, list_response_path_for_spec(&command_spec)).expect("projects array should be extracted");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["id"], json!("project-a"));
    }

    #[test]
    fn extract_provider_items_uses_single_array_fallback() {
        let command_spec = CommandSpec::new_http(
            "vendor".to_string(),
            "rows:list".to_string(),
            "List rows".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/rows", None, None),
            1,
        );
        let payload = json!({
            "meta": { "cursor": "abc" },
            "rows": [{ "id": "row-1" }]
        });

        let items =
            extract_provider_items(payload, list_response_path_for_spec(&command_spec)).expect("single array field should be extracted");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], json!("row-1"));
    }

    #[test]
    fn extract_provider_items_rejects_ambiguous_multi_array_wrappers() {
        let command_spec = CommandSpec::new_http(
            "vendor".to_string(),
            "objects:list".to_string(),
            "List objects".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/objects", None, None),
            1,
        );
        let payload = json!({
            "first": [{ "id": "one" }],
            "other": [{ "id": "other-a" }]
        });

        let items = extract_provider_items(payload, list_response_path_for_spec(&command_spec));
        assert!(items.is_none(), "ambiguous wrappers without list path should fail");
    }

    #[test]
    fn extract_provider_items_extracts_single_object_with_scalar_metadata() {
        let command_spec = CommandSpec::new_http(
            "vendor".to_string(),
            "item:info".to_string(),
            "Get item".to_string(),
            Vec::new(),
            Vec::new(),
            HttpCommandSpec::new("GET", "/v1/item", None, None),
            1,
        );
        let payload = json!({
            "cursor": "abcd",
            "item": { "id": "item-1", "name": "alpha" }
        });

        let items = extract_provider_items(payload, list_response_path_for_spec(&command_spec))
            .expect("single object wrapper should be extracted for providers");
        assert_eq!(items, vec![json!({ "id": "item-1", "name": "alpha" })]);
    }
}
