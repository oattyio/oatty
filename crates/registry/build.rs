use std::{env, fs, path::PathBuf};

use anyhow::Result;

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_path = out_dir.join("heroku-manifest.bin");

    // Source schema: top-level schemas/heroku-schema.json
    let schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("schemas")
        .join("heroku-schema.json");

    println!("cargo:rerun-if-changed={}", schema_path.display());

    // Ensure output directory exists
    if let Some(parent) = manifest_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    heroku_registry_gen::write_manifest(schema_path, manifest_path)
}
