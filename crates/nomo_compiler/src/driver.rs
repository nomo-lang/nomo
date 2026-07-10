use super::*;

pub fn check_source(path: &Path) -> Result<Program, Diagnostic> {
    check_source_with_external_imports(path, &[])
}

pub fn check_source_with_external_imports(
    path: &Path,
    external_import_roots: &[String],
) -> Result<Program, Diagnostic> {
    check_source_with_external_modules(path, external_import_roots, &[])
}

pub fn check_source_with_external_modules(
    path: &Path,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<Program, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    check_source_text_with_project_modules(
        path,
        &source,
        None,
        external_import_roots,
        external_modules,
    )
}

pub fn check_source_text(path: &Path, source: &str) -> Result<Program, Diagnostic> {
    check_source_text_with_external_imports(path, source, &[])
}

pub fn check_source_text_with_external_imports(
    path: &Path,
    source: &str,
    external_import_roots: &[String],
) -> Result<Program, Diagnostic> {
    check_source_text_with_external_modules(path, source, external_import_roots, &[])
}

pub fn check_source_text_with_external_modules(
    path: &Path,
    source: &str,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<Program, Diagnostic> {
    check_source_text_with_project_modules(
        path,
        source,
        None,
        external_import_roots,
        external_modules,
    )
}

pub fn check_source_text_with_project_modules(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<Program, Diagnostic> {
    check_source_text_with_project_modules_and_overrides(
        path,
        source,
        local_source_root,
        external_import_roots,
        external_modules,
        &[],
    )
}

pub fn check_source_text_with_project_modules_and_overrides(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
) -> Result<Program, Diagnostic> {
    let (ast, _, local_import_root) = load_project_modules(
        path,
        source,
        local_source_root,
        external_modules,
        module_source_overrides,
    )?;
    lower_program(
        path,
        ast,
        external_import_roots,
        local_import_root.as_deref(),
        EntryMode::MainFunctionRequired,
    )
}

pub fn build_module_graph(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_modules: &[ExternalModule],
) -> Result<ModuleGraph, Diagnostic> {
    build_module_graph_with_overrides(path, source, local_source_root, external_modules, &[])
}

pub fn build_module_graph_with_overrides(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
) -> Result<ModuleGraph, Diagnostic> {
    load_project_modules(
        path,
        source,
        local_source_root,
        external_modules,
        module_source_overrides,
    )
    .map(|(_, graph, _)| graph)
}

fn load_project_modules(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
) -> Result<(SourceFile, ModuleGraph, Option<String>), Diagnostic> {
    let tokens = lexer::lex(path, source)?;
    let mut ast = parser::parse(path, &tokens)?;
    let local_import_root = local_source_root.and_then(|_| ast.package.first().cloned());
    let root_node = ModuleNode::new(
        ModuleId::from(ast.package.clone()),
        path.to_path_buf(),
        ast.imports.iter().cloned().map(ModuleId::from).collect(),
    );
    let mut module_graph = ModuleGraph::new(root_node);
    merge_imported_public_api(
        path,
        &mut ast,
        local_source_root,
        local_import_root.as_deref(),
        external_modules,
        module_source_overrides,
        &mut module_graph,
    )?;
    Ok((ast, module_graph, local_import_root))
}

pub fn check_script_source_text(path: &Path, source: &str) -> Result<Program, Diagnostic> {
    let tokens = lexer::lex(path, source)?;
    let ast = parser::parse(path, &tokens)?;
    lower_program(path, ast, &[], None, EntryMode::ScriptFile)
}

pub fn compile_source_to_c(path: &Path) -> Result<String, Diagnostic> {
    compile_source_to_c_with_external_imports(path, &[])
}

pub fn compile_script_source_to_c(path: &Path) -> Result<String, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    let program = check_script_source_text(path, &source)?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_to_c_with_external_imports(
    path: &Path,
    external_import_roots: &[String],
) -> Result<String, Diagnostic> {
    let program = check_source_with_external_modules(path, external_import_roots, &[])?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_to_c_with_external_modules(
    path: &Path,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<String, Diagnostic> {
    let program =
        check_source_with_external_modules(path, external_import_roots, external_modules)?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_to_c_with_project_modules(
    path: &Path,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<String, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|err| {
        Diagnostic::new(
            "E0001",
            format!("failed to read source file: {err}"),
            path,
            1,
            1,
            1,
            "",
        )
    })?;
    let program = check_source_text_with_project_modules(
        path,
        &source,
        local_source_root,
        external_import_roots,
        external_modules,
    )?;
    Ok(codegen::emit_c(&program))
}

pub fn compile_source_text_to_c_with_project_modules(
    path: &Path,
    source: &str,
    local_source_root: Option<&Path>,
    external_import_roots: &[String],
    external_modules: &[ExternalModule],
) -> Result<String, Diagnostic> {
    let program = check_source_text_with_project_modules(
        path,
        source,
        local_source_root,
        external_import_roots,
        external_modules,
    )?;
    Ok(codegen::emit_c(&program))
}
