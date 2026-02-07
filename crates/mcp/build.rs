use anyhow::Context;
use oatty_types::workflow::WorkflowDefinition;
use schemars::schema_for;
use std::fs;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=../types/src/workflow.rs");

    let output_directory = PathBuf::from(std::env::var("OUT_DIR").context("OUT_DIR is not set")?);
    let output_path = output_directory.join("workflow_definition.schema.json");

    let schema = schema_for!(WorkflowDefinition);
    let schema_text = serde_json::to_string_pretty(&schema).context("serialize workflow schema")?;
    fs::write(&output_path, schema_text).with_context(|| format!("write schema artifact {}", output_path.display()))?;

    Ok(())
}
