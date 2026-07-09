use std::env;
use std::path::PathBuf;

pub(super) fn parse_doc_args(
    args: Vec<String>,
) -> Result<
    (
        PathBuf,
        bool,
        bool,
        Option<String>,
        bool,
        bool,
        bool,
        Option<PathBuf>,
    ),
    String,
> {
    let usage = "usage: nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]";
    let mut workspace = false;
    let mut package = None;
    let mut include_std = false;
    let mut open = false;
    let mut json = false;
    let mut output = None;
    let mut path = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--workspace" {
            workspace = true;
        } else if let Some(value) = arg.strip_prefix("--package=") {
            if value.is_empty() {
                return Err("--package requires a package id or name".to_string());
            }
            package = Some(value.to_string());
        } else if arg == "--package" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--package requires a package id or name".to_string());
            };
            package = Some(value.clone());
        } else if arg == "--std" {
            include_std = true;
        } else if arg == "--open" {
            open = true;
        } else if arg == "--json" {
            json = true;
        } else if let Some(value) = arg.strip_prefix("--output=") {
            if value.is_empty() {
                return Err("--output requires a directory".to_string());
            }
            output = Some(PathBuf::from(value));
        } else if arg == "--output" {
            index += 1;
            let Some(value) = args.get(index) else {
                return Err("--output requires a directory".to_string());
            };
            output = Some(PathBuf::from(value));
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            return Err(usage.to_string());
        }
        index += 1;
    }
    let path_provided = path.is_some();
    Ok((
        path.unwrap_or(env::current_dir().map_err(|err| err.to_string())?),
        path_provided,
        workspace,
        package,
        include_std,
        open,
        json,
        output,
    ))
}
