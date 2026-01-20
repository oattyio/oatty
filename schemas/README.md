# Schemas

This folder holds OpenAPI documents used for command generation.

Sample OpenAPI specs live under `schemas/samples/` and are used by the registry
generation pipeline during development.

Ways to turn schemas into CLI commands:
- Import an OpenAPI document from the TUI Library view; it generates and saves a registry catalog.
- Programmatically generate a catalog/manifest using the `oatty-registry-gen` crate.

Legacy Heroku schemas have been moved to `legacy/`.
