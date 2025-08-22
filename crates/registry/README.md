Heroku Registry
================

Overview
- The registry crate exposes a `Registry` of Heroku CLI commands derived from the Hyper-Schema.
- Parsing the full schema is expensive at runtime, so the crate now embeds a pre-generated manifest produced at build time.

Build-time Manifest
- Source schema: `schemas/heroku-schema.json`.
- Build script (`build.rs`) generates `OUT_DIR/heroku-manifest.json` using the `heroku-registry-gen` crate.
- The library embeds that manifest with `include_str!` and deserializes it on startup (fast).

Manual Generation
- A helper binary exists for ad-hoc conversion:
  - `cargo run -p heroku-registry-gen --bin schema-to-manifest -- <schema.json> <manifest.json>`

Notes
- Workflow commands are still added at runtime based on `FEATURE_WORKFLOWS`.
- If the schema changes, a rebuild will regenerate the manifest (`cargo:rerun-if-changed`).
