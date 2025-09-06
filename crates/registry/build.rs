use std::{env, fs, path::PathBuf};

use anyhow::Result;
use heroku_registry_gen::io::ManifestInput;
use heroku_types::ServiceId;

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_path = out_dir.join("heroku-manifest.bin");

    // Source schema: top-level schemas/heroku-schema.json
    let heroku_schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("schemas")
        .join("heroku-schema.enhanced.json");
    let data_schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("schemas")
        .join("data-schema.yaml");

    println!("cargo:rerun-if-changed={}", heroku_schema_path.display());
    println!("cargo:rerun-if-changed={}", data_schema_path.display());

    // Ensure output directory exists
    if let Some(parent) = manifest_path.parent()
        && !parent.exists()
    {
        fs::create_dir_all(parent)?;
    }

    heroku_registry_gen::write_manifest(
        vec![
            ManifestInput {
                input: heroku_schema_path,
                service_id: ServiceId::CoreApi,
            },
            ManifestInput {
                input: data_schema_path,
                service_id: ServiceId::DataApi,
            },
        ],
        manifest_path,
    )
}
