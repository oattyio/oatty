use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use clap::Parser;
use heroku_registry_gen::{io::ManifestInput, write_manifest, write_manifest_json};
use heroku_types::ServiceId;

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

    // id of the base url to use for the commands
    #[arg(long)]
    service: String,
}

/// CLI entry point
fn main() -> Result<()> {
    let Args {
        input,
        output,
        json,
        service,
    } = Args::parse();
    let service_id: ServiceId = ServiceId::from_str(&service).ok().unwrap_or(ServiceId::default());
    let input = vec![ManifestInput{input, service_id}];
    if json {
        write_manifest_json(input, output)
    } else {
        write_manifest(input, output)
    }
}
