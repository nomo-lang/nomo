#![allow(clippy::result_large_err, clippy::type_complexity)]

#[path = "nomo_cli/cache.rs"]
mod cli_cache;
#[path = "nomo_cli/common.rs"]
mod cli_common;
#[path = "nomo_cli/deps.rs"]
mod cli_deps;
#[path = "nomo_cli/doc.rs"]
mod cli_doc;
#[path = "nomo_cli/ffi.rs"]
mod cli_ffi;
#[path = "nomo_cli/fmt.rs"]
mod cli_fmt;
#[path = "nomo_cli/manifest.rs"]
mod cli_manifest;
#[path = "nomo_cli/registry.rs"]
mod cli_registry;
#[path = "nomo_cli/supply_chain.rs"]
mod cli_supply_chain;
#[path = "nomo_cli/test.rs"]
mod cli_test;

use cli_cache::run_cache_command;
use cli_common::{run_build_command, run_check_command, run_clean_command, run_run_command};
use cli_deps::run_deps_command;
use cli_doc::run_doc_command;
use cli_ffi::run_ffi_command;
use cli_fmt::run_fmt_command;
use cli_manifest::run_manifest_command;
use cli_registry::{
    run_add_command, run_login_command, run_owner_command, run_publish_command, run_remove_command,
    run_search_command, run_yank_command,
};
use cli_supply_chain::run_verify_command;
use cli_test::run_test_command;
use nomo::project::create_project;
use std::env;
use std::process;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().cloned() else {
        print_help();
        return Ok(());
    };
    args.remove(0);

    match command.as_str() {
        "new" => {
            let [name] = args.as_slice() else {
                return Err("usage: nomo new <name>".to_string());
            };
            let project =
                create_project(&env::current_dir().map_err(|err| err.to_string())?, name)?;
            println!("created {}", project.root.display());
            Ok(())
        }
        "check" => run_check_command(args),
        "build" => run_build_command(args),
        "run" => run_run_command(args),
        "fmt" => run_fmt_command(args),
        "manifest" => run_manifest_command(args),
        "test" => run_test_command(args),
        "doc" => run_doc_command(args),
        "clean" => run_clean_command(args),
        "cache" => run_cache_command(args),
        "login" => run_login_command(args),
        "owner" => run_owner_command(args),
        "add" => run_add_command(args),
        "remove" => run_remove_command(args),
        "search" => run_search_command(args),
        "yank" => run_yank_command(args),
        "publish" => run_publish_command(args),
        "deps" => run_deps_command(args),
        "ffi" => run_ffi_command(args),
        "verify" => run_verify_command(args),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn print_help() {
    println!(
        "nomo {}\n\nCommands:\n  nomo new <name>\n  nomo check [path] [--json-errors] [--workspace]\n  nomo build [path] [--target <triple>] [--emit-c] [--json-errors] [--workspace] [--locked] [--offline] [--frozen]\n  nomo run [path] [--json-errors] [-- args...]\n  nomo fmt [path] [--check] [--json-errors]\n  nomo manifest migrate [path] [--check]\n  nomo test [path] [--workspace] [--package <package>] [--filter <text>] [--json] [--locked] [--offline] [--frozen]\n  nomo doc [path] [--workspace] [--package <package>] [--std] [--open] [--json] [--output <dir>]\n  nomo clean [path]\n  nomo cache stats [path]\n  nomo cache clean [path]\n  nomo cache prune [path] --max-bytes <bytes>\n  nomo login --registry <url> --token <token>\n  nomo owner add <owner/package> <user> --registry <url>\n  nomo owner remove <owner/package> <user> --registry <url>\n  nomo add <alias>@<owner>/<package>:<version> [path] [--registry <url>]\n  nomo remove <alias> [path]\n  nomo search <query> --registry <url>\n  nomo yank <owner/package> <version> --registry <url>\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--json-errors]\n  nomo deps resolve [path] [--workspace] [--locked] [--offline] [--frozen]\n  nomo deps tree [path] [--workspace] [--target <triple>] [--locked] [--offline] [--frozen]\n  nomo deps update [path] [alias-or-package] [--workspace] [--offline] [--precise <version-or-rev>]\n  nomo deps vendor [path] [--workspace] [--dir vendor] [--sync]\n  nomo deps clean-cache [path]\n  nomo ffi bindgen <header> --package <package> --output <file> [--provenance <file>]\n",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "  nomo owner key add <owner/package> <ed25519-public-key-hex> --registry <url>\n  nomo owner key revoke <owner/package> <key-id> --registry <url>\n  nomo publish [path] (--dry-run | --registry <url>) [--output <dir>] [--signer <command>] [--envelope <file>] [--json-errors]\n  nomo verify <archive> --envelope <file> --key <ed25519-public-key-hex> [--provenance <file>] [--transparency <file> --log-key <ed25519-public-key-hex>] [--cached-head <file>] [--gossip <file>] [--write-gossip <file>] [--proof-max-age-seconds <seconds>] [--offline-proof-max-age-seconds <seconds>] [--max-future-skew-seconds <seconds>] [--offline]"
    );
}
