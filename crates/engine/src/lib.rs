//! # Oatty Engine
//!
//! The Oatty Engine parses, validates, and (eventually) executes modern workflow specifications.
//! It provides a robust framework for defining and running multi-step automation workflows
//! with support for conditional execution, input validation, and dynamic value resolution.
//!
//! ## Key Features
//!
//! - **Workflow Parsing**: Parses modern YAML/JSON workflow specs (single or multi-workflow)
//! - **Template Interpolation**: Dynamic value substitution using `${{ ... }}` syntax
//! - **Conditional Execution**: Step-level conditional logic with expression evaluation
//! - **Input Validation**: Declarative input specifications with provider integration
//!
//! ## Usage
//!
//! ```rust
//! use oatty_engine::{parse_workflow_file, WorkflowBundle};
//!
//! // Create a temporary workflow file for testing
//! let temp_dir = tempfile::tempdir()?;
//! let workflow_path = temp_dir.path().join("workflow.yaml");
//! std::fs::write(&workflow_path, r#"
//! workflow: "test-workflow"
//! steps: []
//! "#)?;
//!
//! let workflow_bundle = parse_workflow_file(&workflow_path)?;
//! for (name, spec) in &workflow_bundle.workflows {
//!     println!("Workflow: {}", name);
//!     println!("Steps: {}", spec.steps.len());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Architecture
//!
//! The engine is organized into several key modules:
//!
//! - **`model`**: Core data structures for workflows, steps, and inputs
//! - **`resolve`**: Template interpolation and expression evaluation
//! - **`provider`**: Provider registry and value fetching abstractions
//! - **`executor`**: Workflow execution engine (planned)
//! - **`validator`**: Workflow validation and schema checking (planned)

use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

pub mod executor;
pub mod model;
pub mod provider;
pub mod resolve;
pub mod workflow;

// Re-export commonly used types for convenience
pub use executor::{
    CommandRunner, PreparedStep, RegistryCommandRunner, StepResult, StepStatus, execute_workflow, execute_workflow_with_runner,
};
pub use model::{InputSpec, StepSpec, WorkflowBundle, WorkflowSpec};
pub use provider::{ProviderValueResolver, ValueProvider};
pub use resolve::RunContext;
pub use workflow::bindings::{
    ArgumentPrompt, BindingFailure, BindingSource, MissingReason, ProviderArgumentResolver, ProviderBindingOutcome, SkipDecision,
};
pub use workflow::runner::drive_workflow_run;
pub use workflow::state::{
    InputProviderState, ProviderOutcomeState, ProviderResolutionEvent, ProviderResolutionSource, StepTelemetryEvent, WorkflowRunState,
    WorkflowTelemetry,
};

/// Loads a workflow file from the filesystem with automatic format detection.
///
/// This function attempts to parse the file as either YAML or JSON based on
/// the file extension. For files without extensions, it defaults to YAML.
///
/// # Arguments
///
/// * `file_path` - Path to the workflow file to load
///
/// # Returns
///
/// Returns a `Result<WorkflowBundle>` containing the parsed workflows or an error
/// if parsing fails.
///
/// # Errors
///
/// This function will return an error if:
/// - The file cannot be read from the filesystem
/// - The file content is not valid YAML or JSON
/// - The file structure does not match expected workflow formats
///
/// # Examples
///
/// ```rust
/// use oatty_engine::parse_workflow_file;
///
/// // Create a temporary workflow file for testing
/// let temp_dir = tempfile::tempdir()?;
/// let workflow_path = temp_dir.path().join("deploy.yaml");
/// std::fs::write(&workflow_path, r#"
/// workflow: "deploy-app"
/// steps: []
/// "#)?;
///
/// let workflow_bundle = parse_workflow_file(&workflow_path)?;
/// println!("Loaded {} workflows", workflow_bundle.workflows.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_workflow_file(file_path: impl AsRef<Path>) -> Result<WorkflowBundle> {
    let file_path = file_path.as_ref();
    let file_content = fs::read(file_path).with_context(|| format!("Failed to read workflow file: {}", file_path.display()))?;

    let content_string = String::from_utf8_lossy(&file_content);

    // Attempt to parse as multi-workflow document first to avoid accepting
    // multi documents as single-workflow specs with ignored fields.
    #[derive(Deserialize)]
    struct MultiWorkflowDocument {
        workflows: HashMap<String, WorkflowSpec>,
    }

    if let Ok(multi_workflow_document) = serde_yaml::from_str::<MultiWorkflowDocument>(&content_string) {
        return Ok(WorkflowBundle {
            workflows: multi_workflow_document.workflows,
        });
    }

    // Attempt to parse as single workflow specification
    if let Ok(workflow_specification) = serde_yaml::from_str::<WorkflowSpec>(&content_string) {
        let workflow_name = workflow_specification.workflow.clone().unwrap_or_else(|| "default".to_string());

        let mut workflows = HashMap::new();
        workflows.insert(workflow_name, workflow_specification);

        return Ok(WorkflowBundle { workflows });
    }

    // If none of the parsing attempts succeeded, return a detailed error
    anyhow::bail!(
        "Unsupported workflow document format. Expected one of:\n\
         - Single workflow specification with 'workflow', 'inputs', and 'steps' fields\n\
         - Multi-workflow document with workflows under 'workflows' key\n\
         "
    );
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_workflow_file_single_workflow() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workflow_path = temp_dir.path().join("workflow.yaml");

        let workflow_content = r#"
workflow: "test-workflow"
inputs:
  app_name:
    description: "Application name"
steps:
  - id: "deploy"
    run: "apps:deploy"
"#;

        fs::write(&workflow_path, workflow_content).unwrap();

        let result = parse_workflow_file(&workflow_path);
        assert!(result.is_ok());

        let workflow_bundle = result.unwrap();
        assert_eq!(workflow_bundle.workflows.len(), 1);
        assert!(workflow_bundle.workflows.contains_key("test-workflow"));
    }

    #[test]
    fn test_parse_workflow_file_multi_workflow() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workflow_path = temp_dir.path().join("bundle.yaml");

        let workflow_content = r#"
workflows:
  deploy:
    workflow: "deploy-app"
    steps: []
  rollback:
    workflow: "rollback-app"
    steps: []
"#;

        fs::write(&workflow_path, workflow_content).unwrap();

        let bundle = parse_workflow_file(&workflow_path).expect("parse multi-workflow bundle");
        assert_eq!(bundle.workflows.len(), 2);
        assert!(bundle.workflows.contains_key("deploy"));
        assert!(bundle.workflows.contains_key("rollback"));
        assert_eq!(bundle.workflows["deploy"].workflow.as_deref(), Some("deploy-app"));
        assert_eq!(bundle.workflows["rollback"].workflow.as_deref(), Some("rollback-app"));
    }
}
