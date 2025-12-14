use std::{path::PathBuf, str::FromStr};

use anyhow::Result;
use clap::Parser;
use oatty_registry_gen::{io::ManifestInput, write_manifest, write_manifest_json};
use oatty_types::ServiceId;

/// Command-line arguments structure for `heroku-registry-gen` application.
///
/// This struct is used to parse and validate the command-line arguments provided
/// to the `heroku-registry-gen` tool. The tool is used for generating a manifest
/// from a Oatty JSON Hyper-Schema, with optional customizations such as the output
/// format and workflow definitions.
///
/// # Fields
///
/// * `input` - A required path to the Oatty JSON Hyper-Schema file which will
///   serve as the input for the manifest generation process.
///
/// * `output` - A required path where the generated manifest will be written
///   after the processing.
///
/// * `json` - A flag (boolean) that controls the output format. When provided,
///   the generated manifest will be written as a JSON file. If this flag is
///   omitted, the default format will be bincode (binary encoding).
///
/// * `service` - A user-specified string that represents the ID of the base
///   URL to use for the commands. This field is required and customizes the
///   base service of the generated commands.
///
/// * `workflows` - An optional path to a file containing workflow definitions,
///   which can be in YAML or JSON format. This parameter allows for additional
///   workflow context to be included during manifest generation.
///
/// # Example Usage
///
/// ```bash
/// heroku-registry-gen --input schema.json --output manifest.bin \
///     --service my-service-id --workflows workflows.yaml
///
/// heroku-registry-gen --input schema.json --output manifest.json \
///     --json --service other-service-id
/// ```
///
/// # Attributes
///
/// * The struct derives `Parser` from the `clap` crate, enabling automatic
///   parsing and validation of command-line arguments.
/// * The `Debug` trait is derived for easier debugging of parsed arguments.
/// * `command` attributes provide the program's metadata such as name, version,
///   and description, which are displayed in help messages.
#[derive(Parser, Debug)]
#[command(name = "heroku-registry-gen", version, about)]
struct Args {
    /// Input path to the Oatty JSON Hyper-Schema
    input: PathBuf,

    /// Output path for the generated manifest
    output: PathBuf,

    /// Write JSON instead of bincode
    #[arg(long)]
    json: bool,

    // id of the base url to use for the commands
    #[arg(long)]
    service: String,

    /// Optional path to workflow definitions (YAML/JSON files)
    #[arg(long)]
    workflows: Option<PathBuf>,
}

///
/// Entry point for the application.
///
/// This function parses command-line arguments, processes input data,
/// and generates a manifest file, either in JSON format or a default format,
/// based on the provided options.
///
/// # Command-Line Arguments
/// - `input`: Path to the input file or directory. This field is processed to
///   determine the source data for the manifest.
/// - `output`: Path to the output file or directory where the generated manifest
///   will be saved.
/// - `json`: A boolean flag indicating whether the manifest should be generated
///   in JSON format. If `true`, the manifest will be outputted in JSON format;
///   otherwise, it will use the default format.
/// - `service`: A string identifying the service ID to be associated with this
///   operation. If the provided service string is invalid, a default `ServiceId`
///   will be used.
/// - `workflows`: Any additional workflow-related data to be included in the
///   manifest generation process.
///
/// # Returns
/// A `Result` indicating success or failure. If the process completes without
/// errors, the function will return `Ok(())`. Otherwise, it will return an
/// appropriate error.
///
/// # Functionality
/// 1. Parses the command-line arguments and extracts the required input.
/// 2. Converts the provided service string into a `ServiceId`. If the conversion
///    fails, the default `ServiceId` is used.
/// 3. Constructs the input data as a vector of `ManifestInput` objects.
/// 4. Based on the value of the `json` flag:
///    - If `true`, calls `write_manifest_json` to generate the manifest in JSON
///      format.
///    - Otherwise, calls `write_manifest` to generate the manifest in its default format.
///
/// # Errors
/// This function may fail if any of the following occurs:
/// - Failed file I/O operations when reading input or writing output files.
/// - Invalid data formatting or content issues in the input file.
/// - Any other error propagated by the called functions `write_manifest_json`
///   or `write_manifest`.
///
/// # Example
/// Running the application with appropriate command-line arguments:
/// ```shell
/// my_application --input input_path --output output_path --json --service "MyService" --workflows workflows_data
/// ```
///
/// This would generate a JSON manifest in the specified output directory
/// for the given input and workflows.
///
/// # Dependencies
/// - `Args::parse`: Parses the command-line arguments into a structured `Args` object.
/// - `write_manifest_json` and `write_manifest`: Responsible for the actual
///   manifest file generation in JSON or default format respectively.
///
/// Note: Ensure proper error handling at the calling program level to manage
/// any returned failures effectively.
///
fn main() -> Result<()> {
    let Args {
        input,
        output,
        json,
        service,
        workflows,
    } = Args::parse();
    let service_id: ServiceId = ServiceId::from_str(&service).ok().unwrap_or(ServiceId::default());
    let input = vec![ManifestInput { input, service_id }];
    if json {
        write_manifest_json(input, workflows, output)
    } else {
        write_manifest(input, workflows, output)
    }
}
