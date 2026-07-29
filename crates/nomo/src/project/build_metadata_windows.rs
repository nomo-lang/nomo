use super::{BuildError, command_text, sha256_bytes, sha256_file};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::env;
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::ptr;

const WINDOWS_CANONICAL_PATHEXT: &[&str] = &[".COM", ".EXE", ".BAT", ".CMD"];

pub(super) fn canonical_windows_build_support()
-> Result<(BTreeMap<String, String>, Value), BuildError> {
    let system_directory = windows_system_directory()?;
    let system_root = system_directory.parent().ok_or_else(|| {
        BuildError::Message("GetSystemDirectoryW returned a rootless path".to_string())
    })?;
    let temp_directory = windows_temp_directory()?;
    let minimal_environment =
        canonical_windows_seed_environment(system_root, &system_directory, &temp_directory)?;
    let cmd = PathBuf::from(
        minimal_environment
            .get("COMSPEC")
            .expect("canonical seed environment must contain COMSPEC"),
    );
    let mut program_roots = Vec::new();
    for name in ["ProgramFiles(x86)", "ProgramFiles", "ProgramW6432"] {
        let root = PathBuf::from(
            minimal_environment
                .get(name)
                .ok_or_else(|| BuildError::Message(format!("{name} is unavailable")))?,
        );
        if !program_roots.contains(&root) {
            program_roots.push(root);
        }
    }
    let vswhere = program_roots
        .iter()
        .map(|root| {
            root.join("Microsoft Visual Studio")
                .join("Installer")
                .join("vswhere.exe")
        })
        .find(|path| path.is_file())
        .ok_or_else(|| {
            BuildError::Message(
                "trusted Visual Studio Installer or cmd.exe is unavailable".to_string(),
            )
        })?;
    let vswhere = canonical_path(&vswhere)?;
    let vswhere_text = path_text(&vswhere);
    let vswhere_argv = vec![
        vswhere_text.clone(),
        "-all".to_string(),
        "-products".to_string(),
        "*".to_string(),
        "-requires".to_string(),
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64".to_string(),
        "-format".to_string(),
        "json".to_string(),
        "-utf8".to_string(),
    ];
    let vswhere_output = run_with_environment(&vswhere_argv, &minimal_environment)?;
    require_success(&vswhere_argv, &vswhere_output, "vswhere discovery")?;
    let installations = serde_json::from_slice::<Value>(strip_utf8_bom(&vswhere_output.stdout))
        .map_err(|error| {
            BuildError::Message(format!("vswhere did not return canonical JSON: {error}"))
        })?;
    let (installation, installation_candidates, selection_reason) =
        select_visual_studio_installation(&installations)?;
    let installation_path = canonical_path(Path::new(required_json_string(
        &installation,
        "installationPath",
    )?))?;
    let installation_version =
        required_json_string(&installation, "installationVersion")?.to_string();
    let vsdevcmd = installation_path
        .join("Common7")
        .join("Tools")
        .join("VsDevCmd.bat");
    if !vsdevcmd.is_file() {
        return Err(BuildError::Message(format!(
            "trusted VsDevCmd is unavailable: {}",
            vsdevcmd.display()
        )));
    }
    let vsdevcmd = canonical_path(&vsdevcmd)?;
    let architecture = if env::var("PROCESSOR_ARCHITECTURE")
        .unwrap_or_default()
        .eq_ignore_ascii_case("ARM64")
    {
        "arm64"
    } else {
        "amd64"
    };
    let vsdevcmd_argv = vec![
        path_text(&cmd),
        "/d".to_string(),
        "/c".to_string(),
        "call".to_string(),
        path_text(&vsdevcmd),
        "-no_logo".to_string(),
        format!("-arch={architecture}"),
        format!("-host_arch={architecture}"),
        "&&".to_string(),
        "set".to_string(),
    ];
    let vsdevcmd_output = run_with_environment(&vsdevcmd_argv, &minimal_environment)?;
    require_success(&vsdevcmd_argv, &vsdevcmd_output, "VsDevCmd discovery")?;
    let mut discovered = BTreeMap::new();
    for line in String::from_utf8_lossy(&vsdevcmd_output.stdout).lines() {
        if let Some((key, value)) = line.split_once('=') {
            discovered.insert(key.to_ascii_uppercase(), value.to_string());
        }
    }
    for required in ["PATH", "INCLUDE", "LIB", "LIBPATH"] {
        if discovered.get(required).is_none_or(String::is_empty) {
            return Err(BuildError::Message(
                "VsDevCmd did not publish complete compiler paths".to_string(),
            ));
        }
    }
    let selected_names = [
        "INCLUDE",
        "LIB",
        "LIBPATH",
        "PATH",
        "UCRTVERSION",
        "UNIVERSALCRTSDKDIR",
        "VCINSTALLDIR",
        "VCTOOLSINSTALLDIR",
        "VCTOOLSVERSION",
        "VSCMD_ARG_TGT_ARCH",
        "VSINSTALLDIR",
        "WINDOWSSDKDIR",
        "WINDOWSSDKVERSION",
    ];
    let mut environment = BTreeMap::new();
    for name in selected_names {
        if let Some(value) = discovered.get(name).filter(|value| !value.is_empty()) {
            environment.insert(name.to_string(), value.clone());
        }
    }
    for (name, value) in &minimal_environment {
        if name != "PATH" {
            environment.insert(name.clone(), value.clone());
        }
    }

    let mut llvm_directories = Vec::new();
    for root in &program_roots {
        let llvm = root.join("LLVM").join("bin");
        if llvm.join("clang.exe").is_file() && llvm.join("clang++.exe").is_file() {
            let llvm = canonical_path(&llvm)?;
            if !llvm_directories
                .iter()
                .any(|current: &PathBuf| path_text(current).eq_ignore_ascii_case(&path_text(&llvm)))
            {
                llvm_directories.push(llvm);
            }
        }
    }
    if llvm_directories.len() != 1 {
        return Err(BuildError::Message(
            "trusted Windows LLVM discovery requires exactly one Program Files LLVM driver directory"
                .to_string(),
        ));
    }
    llvm_directories.sort_by_key(|path| path_text(path).to_ascii_lowercase());
    let llvm_directory = llvm_directories
        .into_iter()
        .next()
        .expect("checked one LLVM directory");
    let llvm_tools = BTreeMap::from([
        (
            "clang.exe".to_string(),
            tool_identity(&llvm_directory.join("clang.exe"))?,
        ),
        (
            "clang++.exe".to_string(),
            tool_identity(&llvm_directory.join("clang++.exe"))?,
        ),
    ]);

    let selected_root = canonical_path(&installation_path)?;
    let (vs_path, excluded_path) = filter_paths_within_roots(
        split_windows_paths(
            environment
                .get("PATH")
                .expect("discovered PATH must be present"),
        ),
        std::slice::from_ref(&selected_root),
        "PATH",
        true,
    )?;
    let mut tools = BTreeMap::new();
    for name in ["cl.exe", "link.exe"] {
        let selected = executable_from_path(name, &vs_path, true)?.ok_or_else(|| {
            BuildError::Message(format!("canonical Windows PATH did not select {name}"))
        })?;
        if !path_is_within(&selected, &selected_root) {
            return Err(BuildError::Message(format!(
                "{name} is outside selected Visual Studio"
            )));
        }
        tools.insert(name.to_string(), tool_identity(&selected)?);
    }

    let trusted_program_roots = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .into_iter()
        .map(|name| {
            canonical_path(Path::new(
                minimal_environment
                    .get(name)
                    .expect("canonical seed environment must contain Program Files roots"),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sdk_roots = Vec::new();
    for name in ["WINDOWSSDKDIR", "UNIVERSALCRTSDKDIR"] {
        if let Some(value) = environment.get(name) {
            let path = canonical_path(Path::new(value))?;
            if !trusted_program_roots
                .iter()
                .any(|root| path_is_within(&path, root))
            {
                return Err(BuildError::Message(format!(
                    "{name} is outside trusted Program Files roots: {}",
                    path.display()
                )));
            }
            sdk_roots.push(path);
        }
    }
    if sdk_roots.is_empty() || !environment.contains_key("WINDOWSSDKVERSION") {
        return Err(BuildError::Message(
            "VsDevCmd did not publish a complete Windows SDK".to_string(),
        ));
    }
    let mut allowed_roots = vec![selected_root.clone()];
    allowed_roots.extend(sdk_roots);
    let (include, excluded_include) = filter_paths_within_roots(
        split_windows_paths(
            environment
                .get("INCLUDE")
                .expect("discovered INCLUDE must be present"),
        ),
        &allowed_roots,
        "INCLUDE",
        true,
    )?;
    let (lib, excluded_lib) = filter_paths_within_roots(
        split_windows_paths(
            environment
                .get("LIB")
                .expect("discovered LIB must be present"),
        ),
        &allowed_roots,
        "LIB",
        true,
    )?;
    let (libpath, excluded_libpath) = filter_paths_within_roots(
        split_windows_paths(
            environment
                .get("LIBPATH")
                .expect("discovered LIBPATH must be present"),
        ),
        &allowed_roots,
        "LIBPATH",
        false,
    )?;
    let cl_parent = identity_realpath(
        tools
            .get("cl.exe")
            .expect("selected tools must contain cl.exe"),
    )?
    .parent()
    .expect("cl.exe must have a parent")
    .to_path_buf();
    let link_parent = identity_realpath(
        tools
            .get("link.exe")
            .expect("selected tools must contain link.exe"),
    )?
    .parent()
    .expect("link.exe must have a parent")
    .to_path_buf();
    let mut canonical_path_entries = Vec::new();
    for path in [
        llvm_directory.clone(),
        cl_parent,
        link_parent,
        system_directory,
    ] {
        let value = path_text(&canonical_path(&path)?);
        if !canonical_path_entries.contains(&value) {
            canonical_path_entries.push(value);
        }
    }
    environment.insert("PATH".to_string(), canonical_path_entries.join(";"));
    environment.insert("INCLUDE".to_string(), include.join(";"));
    environment.insert("LIB".to_string(), lib.join(";"));
    environment.insert("LIBPATH".to_string(), libpath.join(";"));

    let sdk_crt_markers = BTreeMap::from([
        (
            "ucrt_header".to_string(),
            authority_marker(&include, "ctype.h", "UCRT ctype.h")?,
        ),
        (
            "windows_header".to_string(),
            authority_marker(&include, "windows.h", "Windows SDK windows.h")?,
        ),
        (
            "sdk_version_header".to_string(),
            authority_marker(&include, "sdkddkver.h", "Windows SDK sdkddkver.h")?,
        ),
        (
            "vc_runtime_header".to_string(),
            authority_marker(&include, "vcruntime.h", "VC runtime vcruntime.h")?,
        ),
        (
            "ucrt_library".to_string(),
            authority_marker(&lib, "ucrt.lib", "UCRT ucrt.lib")?,
        ),
        (
            "kernel32_library".to_string(),
            authority_marker(&lib, "kernel32.lib", "Windows SDK kernel32.lib")?,
        ),
        (
            "vc_runtime_library".to_string(),
            authority_marker(&lib, "libcmt.lib", "VC runtime libcmt.lib")?,
        ),
    ]);
    let authority = json!({
        "schema": 1,
        "architecture": architecture,
        "vswhere": tool_identity(&vswhere)?,
        "vswhere_command": stable_command_record(
            &vswhere_argv,
            &vswhere_output,
            &minimal_environment,
        ),
        "vswhere_stdout": raw_command_stream(&vswhere_output.stdout),
        "vswhere_stderr": raw_command_stream(&vswhere_output.stderr),
        "installation_path": path_text(&installation_path),
        "installation_version": installation_version,
        "installation_candidates": installation_candidates,
        "installation_selection_reason": selection_reason,
        "chosen_installation_json": installation,
        "vsdevcmd": tool_identity(&vsdevcmd)?,
        "vsdevcmd_command": stable_command_record(
            &vsdevcmd_argv,
            &vsdevcmd_output,
            &minimal_environment,
        ),
        "windows_sdk_version": environment.get("WINDOWSSDKVERSION"),
        "vc_tools_version": environment.get("VCTOOLSVERSION"),
        "tools": tools,
        "llvm_tools": llvm_tools,
        "sdk_crt_markers": sdk_crt_markers,
        "excluded_candidates": {
            "path": excluded_path,
            "include": excluded_include,
            "lib": excluded_lib,
            "libpath": excluded_libpath,
        },
        "include": include,
        "lib": lib,
        "libpath": libpath,
        "path": canonical_path_entries,
    });
    for name in ["HOME", "USERPROFILE", "RUSTUP_HOME"] {
        if let Some(value) = env::var_os(name) {
            environment.insert(name.to_string(), value.to_string_lossy().into_owned());
        }
    }
    environment.insert("GOENV".to_string(), "off".to_string());
    environment.insert("LC_ALL".to_string(), "C".to_string());
    environment.insert("LANG".to_string(), "C".to_string());
    Ok((environment, authority))
}

fn canonical_windows_seed_environment(
    system_root: &Path,
    system_directory: &Path,
    temp_directory: &Path,
) -> Result<BTreeMap<String, String>, BuildError> {
    let program_data = windows_known_folder(Guid::new(
        0x62ab5d82,
        0xfdc1,
        0x4dc3,
        [0xa9, 0xdd, 0x07, 0x0d, 0x1d, 0x49, 0x5d, 0x97],
    ))?;
    let program_files = windows_known_folder(Guid::new(
        0x905e63b6,
        0xc1bf,
        0x494e,
        [0xb2, 0x9c, 0x65, 0xb7, 0x32, 0xd3, 0xd2, 0x1a],
    ))?;
    let program_files_x86 = windows_known_folder(Guid::new(
        0x7c5a40ef,
        0xa0fb,
        0x4bfc,
        [0x87, 0x4a, 0xc0, 0xf2, 0xe0, 0xb9, 0xfa, 0x8e],
    ))?;
    let program_files_x64 = windows_known_folder(Guid::new(
        0x6d809377,
        0x6af0,
        0x444b,
        [0x89, 0x57, 0xa3, 0x77, 0x3f, 0x02, 0x20, 0x0e],
    ))?;
    let cmd = canonical_path(&system_directory.join("cmd.exe"))?;
    if !cmd.is_file() {
        return Err(BuildError::Message(format!(
            "trusted COMSPEC is unavailable: {}",
            cmd.display()
        )));
    }
    let architecture = if env::var("PROCESSOR_ARCHITECTURE")
        .unwrap_or_default()
        .eq_ignore_ascii_case("ARM64")
    {
        "ARM64"
    } else {
        "AMD64"
    };
    Ok(BTreeMap::from([
        ("SystemRoot".to_string(), path_text(system_root)),
        ("WINDIR".to_string(), path_text(system_root)),
        ("TEMP".to_string(), path_text(temp_directory)),
        ("TMP".to_string(), path_text(temp_directory)),
        ("ProgramData".to_string(), path_text(&program_data)),
        ("ProgramFiles".to_string(), path_text(&program_files)),
        (
            "ProgramFiles(x86)".to_string(),
            path_text(&program_files_x86),
        ),
        ("ProgramW6432".to_string(), path_text(&program_files_x64)),
        ("COMSPEC".to_string(), path_text(&cmd)),
        ("PATHEXT".to_string(), WINDOWS_CANONICAL_PATHEXT.join(";")),
        (
            "PROCESSOR_ARCHITECTURE".to_string(),
            architecture.to_string(),
        ),
        ("PATH".to_string(), path_text(system_directory)),
    ]))
}

fn select_visual_studio_installation(
    installations: &Value,
) -> Result<(Value, Vec<Value>, String), BuildError> {
    let values = installations.as_array().ok_or_else(|| {
        BuildError::Message("vswhere did not return an installation array".to_string())
    })?;
    let mut candidates = Vec::new();
    let mut eligible = Vec::new();
    for raw in values {
        let object = raw.as_object().ok_or_else(|| {
            BuildError::Message("vswhere returned a non-object installation".to_string())
        })?;
        let raw_path = object
            .get("installationPath")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = object
            .get("installationVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let product_id = object
            .get("productId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let product_line = object
            .get("catalog")
            .and_then(Value::as_object)
            .and_then(|catalog| catalog.get("productLine"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut reasons = Vec::new();
        let path = PathBuf::from(raw_path);
        let canonical = canonical_path(&path).ok();
        if !path.is_absolute() || canonical.as_ref().is_none_or(|path| !path.is_dir()) {
            reasons.push("installation path is unavailable".to_string());
        }
        let version_parts = version
            .split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>();
        if version.is_empty() || version_parts.is_err() {
            reasons.push("installation version is not numeric".to_string());
        }
        if object.get("isComplete") != Some(&Value::Bool(true)) {
            reasons.push("installation is not complete".to_string());
        }
        if object.get("isLaunchable") != Some(&Value::Bool(true)) {
            reasons.push("installation is not launchable".to_string());
        }
        if object.get("isPrerelease") != Some(&Value::Bool(false)) {
            reasons.push("installation is prerelease or unspecified".to_string());
        }
        if product_line != "Dev17" {
            reasons.push("installation productLine is not Visual Studio 2022".to_string());
        }
        let install_date = object.get("installDate").cloned().unwrap_or(Value::Null);
        let install_date_text = install_date.as_str().unwrap_or_default();
        if !install_date_text.ends_with('Z') || !install_date_text.contains('T') {
            reasons.push("installation installDate is not canonical UTC".to_string());
        }
        let vsdevcmd = path.join("Common7").join("Tools").join("VsDevCmd.bat");
        if !vsdevcmd.is_file() {
            reasons.push("VsDevCmd.bat is unavailable".to_string());
        }
        let installation_path = canonical
            .as_ref()
            .map(|path| path_text(path))
            .unwrap_or_else(|| raw_path.to_string());
        let record = json!({
            "installation_path": installation_path,
            "installation_version": version,
            "product_id": product_id,
            "product_line": product_line,
            "install_date": install_date,
            "is_complete": object.get("isComplete").cloned().unwrap_or(Value::Null),
            "is_launchable": object.get("isLaunchable").cloned().unwrap_or(Value::Null),
            "is_prerelease": object.get("isPrerelease").cloned().unwrap_or(Value::Null),
            "eligible": reasons.is_empty(),
            "reasons": reasons,
        });
        if reasons.is_empty() {
            eligible.push((
                version_parts.expect("eligible version must be numeric"),
                install_date_text.to_string(),
                installation_path.to_ascii_lowercase(),
                raw.clone(),
            ));
        }
        candidates.push(record);
    }
    if eligible.is_empty() {
        return Err(BuildError::Message(
            "vswhere found no complete Visual Studio with VC tools".to_string(),
        ));
    }
    let newest_version = eligible
        .iter()
        .map(|item| item.0.clone())
        .max()
        .expect("eligible installation exists");
    let newest_date = eligible
        .iter()
        .filter(|item| item.0 == newest_version)
        .map(|item| item.1.clone())
        .max()
        .expect("newest version exists");
    let mut newest = eligible
        .into_iter()
        .filter(|item| item.0 == newest_version && item.1 == newest_date)
        .collect::<Vec<_>>();
    newest.sort_by(|left, right| left.2.cmp(&right.2));
    if newest.windows(2).any(|pair| pair[0].2 == pair[1].2) {
        return Err(BuildError::Message(
            "vswhere returned ambiguous top-ranked Visual Studio records".to_string(),
        ));
    }
    let selected = newest
        .into_iter()
        .next()
        .expect("newest installation exists")
        .3;
    let selected_path = path_text(&canonical_path(Path::new(required_json_string(
        &selected,
        "installationPath",
    )?))?);
    candidates.sort_by(|left, right| {
        let left_version = left["installation_version"].as_str().unwrap_or_default();
        let right_version = right["installation_version"].as_str().unwrap_or_default();
        left_version.cmp(right_version).then_with(|| {
            left["installation_path"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .cmp(
                    &right["installation_path"]
                        .as_str()
                        .unwrap_or_default()
                        .to_ascii_lowercase(),
                )
        })
    });
    Ok((
        selected,
        candidates,
        format!(
            "selected complete, launchable, non-prerelease Visual Studio 2022 by numeric installationVersion descending, installDate descending, then lexicographically smallest canonical installation path ({selected_path})"
        ),
    ))
}

fn filter_paths_within_roots(
    values: Vec<String>,
    roots: &[PathBuf],
    label: &str,
    require_nonempty: bool,
) -> Result<(Vec<String>, Vec<Value>), BuildError> {
    let roots = roots
        .iter()
        .map(|root| canonical_path(root))
        .collect::<Result<Vec<_>, _>>()?;
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    for value in values.into_iter().filter(|value| !value.is_empty()) {
        let path = match canonical_path(Path::new(&value)) {
            Ok(path) if path.is_dir() => path,
            _ => {
                excluded.push(json!({
                    "path": value,
                    "reason": "directory is unavailable",
                }));
                continue;
            }
        };
        let text = path_text(&path);
        if !roots.iter().any(|root| path_is_within(&path, root)) {
            excluded.push(json!({
                "path": text,
                "reason": "outside selected VS VC and Windows SDK/UCRT roots",
            }));
            continue;
        }
        if !included.contains(&text) {
            included.push(text);
        }
    }
    if require_nonempty && included.is_empty() {
        return Err(BuildError::Message(format!(
            "{label} did not contain any canonical paths"
        )));
    }
    Ok((included, excluded))
}

fn executable_from_path(
    name: &str,
    directories: &[String],
    require_unique: bool,
) -> Result<Option<PathBuf>, BuildError> {
    if Path::new(name).file_name().and_then(|value| value.to_str()) != Some(name) {
        return Err(BuildError::Message(format!(
            "canonical Windows executable lookup requires a basename: {name}"
        )));
    }
    let candidate_names = if Path::new(name).extension().is_some() {
        vec![name.to_string()]
    } else {
        WINDOWS_CANONICAL_PATHEXT
            .iter()
            .map(|extension| format!("{name}{extension}"))
            .collect()
    };
    let mut matches = Vec::new();
    for directory in directories {
        for candidate_name in &candidate_names {
            let candidate = Path::new(directory).join(candidate_name);
            if candidate.is_file() {
                let canonical = canonical_path(&candidate)?;
                if !matches.contains(&canonical) {
                    matches.push(canonical);
                }
            }
        }
    }
    if require_unique && matches.len() != 1 {
        return Err(BuildError::Message(format!(
            "canonical Windows PATH must select exactly one {name}; found {}",
            matches.len()
        )));
    }
    Ok(matches.into_iter().next())
}

fn authority_marker(
    directories: &[String],
    filename: &str,
    label: &str,
) -> Result<Value, BuildError> {
    let mut matches = Vec::new();
    for directory in directories {
        let candidate = Path::new(directory).join(filename);
        if candidate.is_file() {
            let candidate = canonical_path(&candidate)?;
            if !matches.contains(&candidate) {
                matches.push(candidate);
            }
        }
    }
    matches.sort_by_key(|path| path_text(path).to_ascii_lowercase());
    if matches.len() != 1 {
        return Err(BuildError::Message(format!(
            "Windows SDK/CRT authority requires exactly one {label} marker; found {}",
            matches.len()
        )));
    }
    let path = &matches[0];
    Ok(json!({
        "path": path_text(path),
        "sha256": sha256_file(path)?,
    }))
}

fn tool_identity(path: &Path) -> Result<Value, BuildError> {
    let invocation = absolute_windows_path(path)?;
    let realpath = canonical_path(&invocation)?;
    if !realpath.is_file() {
        return Err(BuildError::Message(format!(
            "Windows tool is unavailable: {}",
            invocation.display()
        )));
    }
    Ok(json!({
        "path": path_text(&invocation),
        "realpath": path_text(&realpath),
        "sha256": sha256_file(&realpath)?,
    }))
}

fn identity_realpath(identity: &Value) -> Result<PathBuf, BuildError> {
    identity
        .get("realpath")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| BuildError::Message("Windows tool identity lacks realpath".to_string()))
}

fn raw_command_stream(content: &[u8]) -> Value {
    json!({
        "base64": base64_encode(content),
        "length_bytes": content.len(),
        "sha256": sha256_bytes(content),
    })
}

fn stable_command_record(
    argv: &[String],
    output: &Output,
    environment: &BTreeMap<String, String>,
) -> Value {
    json!({
        "argv": argv,
        "command": command_text(argv),
        "cwd": Value::Null,
        "exit_code": output.status.code().unwrap_or(0),
        "environment": environment,
    })
}

fn run_with_environment(
    argv: &[String],
    environment: &BTreeMap<String, String>,
) -> Result<Output, BuildError> {
    Command::new(&argv[0])
        .args(&argv[1..])
        .env_clear()
        .envs(environment)
        .output()
        .map_err(|error| {
            BuildError::Message(format!("failed to run `{}`: {error}", command_text(argv)))
        })
}

fn require_success(argv: &[String], output: &Output, label: &str) -> Result<(), BuildError> {
    if output.status.success() {
        return Ok(());
    }
    Err(BuildError::Message(format!(
        "{label} failed while running `{}`:\n{}{}",
        command_text(argv),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )))
}

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl Guid {
    const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self {
            data1,
            data2,
            data3,
            data4,
        }
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
    fn GetTempPathW(buffer_length: u32, buffer: *mut u16) -> u32;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(
        folder_id: *const Guid,
        flags: u32,
        token: *mut c_void,
        path: *mut *mut u16,
    ) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(memory: *mut c_void);
}

fn windows_system_directory() -> Result<PathBuf, BuildError> {
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if length == 0 || length as usize >= buffer.len() {
        return Err(BuildError::Message(
            "GetSystemDirectoryW failed".to_string(),
        ));
    }
    let directory = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
    if !directory.is_absolute() {
        return Err(BuildError::Message(
            "GetSystemDirectoryW returned a non-absolute path".to_string(),
        ));
    }
    canonical_path(&directory)
}

fn windows_temp_directory() -> Result<PathBuf, BuildError> {
    let mut buffer = vec![0_u16; 32_768];
    let length = unsafe { GetTempPathW(buffer.len() as u32, buffer.as_mut_ptr()) };
    if length == 0 || length as usize >= buffer.len() {
        return Err(BuildError::Message("GetTempPathW failed".to_string()));
    }
    let directory = PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]));
    if !directory.is_absolute() || !directory.is_dir() {
        return Err(BuildError::Message(
            "GetTempPathW returned an unavailable path".to_string(),
        ));
    }
    canonical_path(&directory)
}

fn windows_known_folder(folder_id: Guid) -> Result<PathBuf, BuildError> {
    let mut value = ptr::null_mut::<u16>();
    let result = unsafe { SHGetKnownFolderPath(&folder_id, 0, ptr::null_mut(), &mut value) };
    if result != 0 || value.is_null() {
        if !value.is_null() {
            unsafe { CoTaskMemFree(value.cast()) };
        }
        return Err(BuildError::Message(format!(
            "SHGetKnownFolderPath failed with HRESULT {result}"
        )));
    }
    let length = (0..32_768)
        .find(|index| unsafe { *value.add(*index) } == 0)
        .ok_or_else(|| {
            unsafe { CoTaskMemFree(value.cast()) };
            BuildError::Message("SHGetKnownFolderPath returned an unterminated path".to_string())
        })?;
    let path = PathBuf::from(String::from_utf16_lossy(unsafe {
        std::slice::from_raw_parts(value, length)
    }));
    unsafe { CoTaskMemFree(value.cast()) };
    if !path.is_absolute() || !path.is_dir() {
        return Err(BuildError::Message(format!(
            "Known Folder path is unavailable: {}",
            path.display()
        )));
    }
    canonical_path(&path)
}

fn required_json_string<'a>(value: &'a Value, name: &str) -> Result<&'a str, BuildError> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| BuildError::Message(format!("Visual Studio installation lacks {name}")))
}

fn split_windows_paths(value: &str) -> Vec<String> {
    value
        .split(';')
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn absolute_windows_path(path: &Path) -> Result<PathBuf, BuildError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(|error| BuildError::Message(error.to_string()))?
            .join(path))
    }
}

fn canonical_path(path: &Path) -> Result<PathBuf, BuildError> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        BuildError::Message(format!("failed to resolve {}: {error}", path.display()))
    })?;
    let text = canonical.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        Ok(PathBuf::from(format!(r"\\{rest}")))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        Ok(PathBuf::from(rest))
    } else {
        Ok(canonical)
    }
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    let normalize = |value: &Path| {
        path_text(value)
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    };
    let path = normalize(path);
    let root = normalize(root);
    path == root
        || path
            .strip_prefix(&root)
            .is_some_and(|rest| rest.starts_with('\\'))
}

fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(TABLE[(first >> 2) as usize] as char);
        encoded.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn windows_path_containment_is_case_insensitive_and_segment_aware() {
        assert!(path_is_within(
            Path::new(r"C:\Program Files\SDK\Include"),
            Path::new(r"c:\program files\sdk"),
        ));
        assert!(!path_is_within(
            Path::new(r"C:\Program Files\SDK-other"),
            Path::new(r"C:\Program Files\SDK"),
        ));
    }
}
