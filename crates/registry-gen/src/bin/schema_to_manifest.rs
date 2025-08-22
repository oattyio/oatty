use anyhow::{Context, Result};
use std::{env, fs, path::PathBuf};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: schema-to-manifest <input_schema.json> <output_manifest.json>");
        std::process::exit(2);
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let schema = fs::read_to_string(&input).with_context(|| format!("read {}", input.display()))?;
    let manifest = heroku_registry_gen::generate_manifest(&schema)?;
    if let Some(parent) = output.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
    }
    fs::write(&output, manifest).with_context(|| format!("write {}", output.display()))?;
    println!("wrote {}", output.display());
    Ok(())
}
