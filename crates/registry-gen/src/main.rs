use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use heroku_registry_gen::{write_manifest, write_manifest_json};

/// Generate the Heroku command registry from a JSON Hyper-Schema.
#[derive(Parser, Debug)]
#[command(name = "heroku-registry-gen", version, about)]
struct Args {
    /// Input path to the Heroku JSON Hyper-Schema
    input: PathBuf,

    /// Output path for the generated manifest
    output: PathBuf,

    /// Write JSON instead of bincode
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.json {
        write_manifest_json(args.input, args.output)
    } else {
        write_manifest(args.input, args.output)
    }
}
