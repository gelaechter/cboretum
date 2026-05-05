use ciborium::into_writer;
use ciborium::value::Value;
use clap::{ArgAction, Parser};
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use log::{LevelFilter, debug, warn};
use std::{collections::HashMap, fs::File, io::BufWriter, path::PathBuf};
use std::{
    fs,
    io::{Write, stdout},
};
use std::{path::Path, result::Result::Ok};

/// A command line utility that allows you to "pack" a directory and it's contents
/// into a CBOR (see https://cbor.io/) representation.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
)]
struct Args {
    /// Only include files up to a certain size
    ///
    /// You may specify a size using equals:
    ///   --max-size      => defaults to 40KiB
    ///   --max-size=1024 => limit of 1024 bytes
    ///   --max-size=3MB  => limit of 3MB
    #[arg(
        short,
        long,
        require_equals=true,
        default_missing_value="40KiB",
        value_parser=parse_byte_size,
        num_args=0..=1,
        verbatim_doc_comment
    )]
    max_size: Option<u64>,
    /// Only include files that are valid UTF-8 text
    ///
    /// On occasion this may include non-text files if
    /// their contents happen to be parsable as UTF-8
    #[arg(short, long)]
    text_only: bool,
    #[arg(short, long)]
    /// Changes leaf content to (filename, content) pairs
    ///
    /// This is useful if you intend to display the filename
    /// along the file contents
    include_file_name: bool,
    /// Increase verbosity (repeat for more: -v info, -vv debug, -vvv trace)
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
    /// Path to the directory to pack
    path: PathBuf,
}

const CLEAR_LINE: &str = "\r\x1B[K";

fn main() -> Result<()> {
    color_eyre::install()?;

    // Check if this is a directory
    let args = Args::parse();
    debug!("Received arguments: {args:?}");

    // Set logger verbosity
    env_logger::Builder::new()
        .filter_level(match args.verbose {
            0 => LevelFilter::Warn,
            1 => LevelFilter::Info,
            2 => LevelFilter::Debug,
            _ => LevelFilter::Trace,
        })
        .init();

    if !args.path.is_dir() {
        return Err(eyre!(format!("{} is not a directory", args.path.display())));
    }

    // Build the file tree
    let tree = build_file_tree(
        Path::new(&args.path),
        args.max_size,
        args.text_only,
        args.include_file_name,
    )?;
    print!("{CLEAR_LINE}");

    // Determine the output file name
    let file_name = format!(
        "{}.cbor",
        args.path
            .canonicalize()
            .ok()
            .and_then(|path| path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()))
            .unwrap_or_else(|| {
                warn!(
                    "Could not canonicalize serialized directory path.\n\
                        Using fallback name..."
                );
                "arbor".to_string()
            })
    );

    let file = File::create(&file_name).wrap_err("Couldn't create output file")?;
    let mut buffered = BufWriter::new(file);
    into_writer(&tree, &mut buffered).unwrap();
    println!("Wrote cbor file to {}", file_name);
    Ok(())
}

fn build_file_tree(
    root: &Path,
    max_size: Option<u64>,
    text_only: bool,
    include_file_path: bool,
) -> Result<Value> {
    let mut map: HashMap<String, Value> = HashMap::new();

    stdout().flush()?;

    let read_dir =
        fs::read_dir(root).wrap_err(format!("Couldn't read directory {}", root.display()))?;

    for entry in read_dir.flatten() {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        let value = if entry_path.is_file() {
            // Turn this file into a value
            read_file_to_value(&entry_path, max_size, text_only, include_file_path)?
        } else if entry_path.is_dir() {
            // Keep building
            build_file_tree(&entry_path, max_size, text_only, include_file_path)?
        } else {
            continue;
        };

        map.insert(name, value);
    }

    Value::serialized(&map).wrap_err(format!("Couldn't serialize to CBOR {map:?}"))
}

/// Read the file at `entry_path` and return a fitting Value based on
/// the given options
fn read_file_to_value(
    entry_path: &PathBuf,
    max_size: Option<u64>,
    text_only: bool,
    with_file_path: bool,
) -> Result<Value, color_eyre::eyre::Error> {
    // There is a limit
    if let Some(max_size) = max_size &&
        // And the file is too large
        {
            let metadata = fs::metadata(entry_path).wrap_err(format!(
                "Couldn't read metadata of file {}",
                entry_path.display()
            ))?;
            metadata.len() > max_size
        }
    {
        // So we skip the file
        debug!("Skipping {} due to max-size", entry_path.display());
        print!("{CLEAR_LINE}Skipping {}...", entry_path.display());
        return Ok(Value::Null);
    }

    // Otherwise, there is no limit so we read the file
    debug!("Reading {}...", entry_path.display());
    print!("{CLEAR_LINE}Reading {}...", entry_path.display());

    let bytes =
        fs::read(entry_path).wrap_err(format!("Couldn't read file {}", entry_path.display()))?;

    // Try to parse it to UTF-8
    let content = match String::from_utf8(bytes) {
        Ok(s) => Value::Text(s),
        // Skip file is not UTF-8 and text_only is active
        Err(e) if !text_only => Value::Bytes(e.into_bytes()),
        _ => {
            debug!("Excluded {} due to text-only", entry_path.display());
            Value::Null
        }
    };

    // Include file path as well if we want that
    Ok(if with_file_path {
        Value::Array(vec![
            Value::Text(entry_path.to_string_lossy().to_string()),
            content,
        ])
    } else {
        content
    })
}

/// Parse either a plain number or a human‑readable byte size.
fn parse_byte_size(s: &str) -> std::result::Result<u64, String> {
    let s = s.trim();

    // Try as plain integer first
    if let Ok(n) = s.parse::<u64>() {
        return Ok(n);
    }

    // Try as human‑readable byte size
    let pos = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let number_str = &s[..pos];
    let unit = &s[pos..];

    let number: u64 = number_str
        .parse()
        .map_err(|_| format!("'{number_str}' is not a valid number"))?;

    let factor = match unit.trim().to_ascii_lowercase().as_str() {
        "k" | "kb" => 1_000,
        "m" | "mb" => 1_000_000,
        "g" | "gb" => 1_000_000_000,
        "ki" | "kib" => 1 << 10,
        "mi" | "mib" => 1 << 20,
        "gi" | "gib" => 1 << 30,
        _ => return Err(format!("'{unit}' is not a valid unit")),
    };

    let bytes = number * factor;
    Ok(bytes)
}
