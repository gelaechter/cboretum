use ciborium::into_writer;
use ciborium::value::Value;
use clap::Parser;
use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use std::{collections::HashMap, fs::File, io::BufWriter, path::PathBuf};
use std::{
    fs,
    io::{Write, stdout},
};
use std::{path::Path, result::Result::Ok};

#[derive(Parser, Debug)]
#[command(author, version, about = "Recursively converts a directory into a CBOR tree", long_about = None)]
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
    /// Only include files that are valid UTF-8
    #[arg(short, long)]
    text_only: bool,
    /// Path to a directory
    path: PathBuf,
}

const CLEAR_LINE: &str = "\r\x1B[K";

fn main() -> Result<()> {
    color_eyre::install()?;

    // Check if this is a directory
    let args = Args::parse();
    if !args.path.is_dir() {
        return Err(eyre!(format!("{} is not a directory", args.path.display())));
    }

    let tree = build_file_tree(Path::new(&args.path), args.max_size, args.text_only)?;
    print!("{CLEAR_LINE}");

    let file_name = format!(
        "{}.cbor",
        args.path
            .canonicalize()
            .ok()
            .and_then(|path| path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()))
            .unwrap_or("default".to_string())
    );

    let file = File::create(&file_name).wrap_err("Couldn't create file")?;
    let mut buffered = BufWriter::new(file);
    into_writer(&tree, &mut buffered).unwrap();
    println!("Wrote file to {}", file_name);
    Ok(())
}

fn build_file_tree(root: &Path, max_size: Option<u64>, text_only: bool) -> Result<Value> {
    let mut map: HashMap<String, Value> = HashMap::new();

    stdout().flush()?;

    let read_dir =
        fs::read_dir(root).wrap_err(format!("Couldn't read directory {}", root.display()))?;
    let mut entries: Vec<(String, Value)> = Vec::new();

    for entry in read_dir.flatten() {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        let value = if entry_path.is_file() {
            match max_size {
                // There is a limit and the file is too large
                Some(max_size)
                    if {
                        let metadata = fs::metadata(&entry_path).wrap_err(format!(
                            "Couldn't read metadata of file {}",
                            entry_path.display()
                        ))?;
                        metadata.len() > max_size
                    } =>
                {
                    // Then skip the file
                    print!("{CLEAR_LINE}Skipping {}...", entry_path.display());
                    Value::Null
                }
                // Otherwise there is no limit
                _ => {
                    // So read the file
                    print!("{CLEAR_LINE}Reading {}...", entry_path.display());

                    let bytes = fs::read(&entry_path)
                        .wrap_err(format!("Couldn't read file {}", entry_path.display()))?;

                    // Try to parse it to UTF-8
                    match String::from_utf8(bytes) {
                        Ok(s) => Value::Text(s),
                        // Skip file if parsing fails and text_only is active
                        Err(e) if !text_only => Value::Bytes(e.into_bytes()),
                        _ => Value::Null,
                    }
                }
            }
        } else if entry_path.is_dir() {
            // Keep building
            build_file_tree(&entry_path, max_size, text_only)?
        } else {
            continue;
        };

        entries.push((name, value));
    }

    for (name, value) in entries {
        map.insert(name, value);
    }

    Value::serialized(&map).wrap_err(format!("Couldn't serialize to CBOR {map:?}"))
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
