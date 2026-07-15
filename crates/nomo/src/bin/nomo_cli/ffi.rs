use nomo::ffi_bindgen::write_bindings_from_header;
use std::path::PathBuf;

const BINDGEN_USAGE: &str =
    "usage: nomo ffi bindgen <header> --package <package> --output <file> [--provenance <file>]";

pub(super) fn run_ffi_command(args: Vec<String>) -> Result<(), String> {
    let Some((subcommand, rest)) = args.split_first() else {
        return Err(BINDGEN_USAGE.to_string());
    };
    match subcommand.as_str() {
        "bindgen" => run_bindgen_command(rest),
        other => Err(format!("unknown ffi command `{other}`")),
    }
}

fn run_bindgen_command(args: &[String]) -> Result<(), String> {
    let (header, package, output, provenance) = parse_bindgen_args(args)?;
    let (output, provenance) =
        write_bindings_from_header(&header, &package, &output, provenance.as_deref())?;
    println!("generated {}", output.display());
    println!("provenance {}", provenance.display());
    Ok(())
}

fn parse_bindgen_args(
    args: &[String],
) -> Result<(PathBuf, String, PathBuf, Option<PathBuf>), String> {
    let mut header = None;
    let mut package = None;
    let mut output = None;
    let mut provenance = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--package" => {
                index += 1;
                package = Some(
                    args.get(index)
                        .cloned()
                        .ok_or_else(|| BINDGEN_USAGE.to_string())?,
                );
            }
            "--output" => {
                index += 1;
                output = Some(PathBuf::from(
                    args.get(index).ok_or_else(|| BINDGEN_USAGE.to_string())?,
                ));
            }
            "--provenance" => {
                index += 1;
                provenance = Some(PathBuf::from(
                    args.get(index).ok_or_else(|| BINDGEN_USAGE.to_string())?,
                ));
            }
            flag if flag.starts_with('-') => {
                return Err(format!("unknown ffi bindgen flag `{flag}`"));
            }
            value if header.is_none() => header = Some(PathBuf::from(value)),
            _ => return Err(BINDGEN_USAGE.to_string()),
        }
        index += 1;
    }

    Ok((
        header.ok_or_else(|| BINDGEN_USAGE.to_string())?,
        package.ok_or_else(|| BINDGEN_USAGE.to_string())?,
        output.ok_or_else(|| BINDGEN_USAGE.to_string())?,
        provenance,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindgen_args_accept_optional_provenance() {
        let args = [
            "api.h",
            "--package",
            "app.bindings",
            "--output",
            "src/bindings.nomo",
            "--provenance",
            "bindings.json",
        ]
        .map(str::to_string);
        let parsed = parse_bindgen_args(&args).unwrap();
        assert_eq!(parsed.0, PathBuf::from("api.h"));
        assert_eq!(parsed.1, "app.bindings");
        assert_eq!(parsed.2, PathBuf::from("src/bindings.nomo"));
        assert_eq!(parsed.3, Some(PathBuf::from("bindings.json")));
    }
}
