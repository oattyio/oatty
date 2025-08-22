use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let manifest_path = out_dir.join("heroku-manifest.json");

    // Source schema: top-level schemas/heroku-schema.json
    let schema_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?)
        .join("..")
        .join("..")
        .join("schemas")
        .join("heroku-schema.json");

    println!("cargo:rerun-if-changed={}", schema_path.display());

    // Ensure output directory exists
    if let Some(parent) = manifest_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let schema = fs::read_to_string(&schema_path)?;
    let manifest = heroku_registry_gen::generate_manifest(&schema)?;
    fs::write(&manifest_path, manifest)?;
    Ok(())
}
