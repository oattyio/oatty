# Workflows

Place curated YAML or JSON workflows here. Files are loaded recursively and must conform to the
schema described in `specs/WORKFLOWS.md`. A minimal example:

```yaml
workflow: app_with_db
inputs:
  app_name:
    description: "Name for new app"
    type: string
    validate:
      pattern: "^[a-z](?:[a-z0-9-]{1,28}[a-z0-9])$"
  region:
    provider: regions list
    select:
      value_field: name
      display_field: name
steps:
  - id: create_app
    run: apps create
    body:
      name: ${{ inputs.app_name }}
      region: ${{ inputs.region }}
  - id: confirm
    run: apps info
    with:
      app: ${{ inputs.app_name }}
```

During the build, every workflow is parsed via `oatty_types::workflow::WorkflowDefinition` and
bundled into the manifest. Validation errors surface with the offending file path.

Legacy Heroku-centric workflow samples have been moved to `legacy/workflows/`.
