# Workflows

Place curated YAML/JSON workflows here. Example structure:

```yaml
steps:
  - id: create_app
    op: http.post
    params:
      path: /apps
      body:
        name: my-app
  - id: set_config
    op: http.patch
    depends_on: [create_app]
    params:
      path: /apps/{{ steps.create_app.response.name }}/config-vars
      body:
        FOO: bar
```

