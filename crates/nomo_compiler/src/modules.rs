use super::*;

pub(super) fn merge_imported_public_api(
    importer_path: &Path,
    ast: &mut SourceFile,
    local_source_root: Option<&Path>,
    local_import_root: Option<&str>,
    external_modules: &[ExternalModule],
    module_source_overrides: &[(PathBuf, String)],
    visited: &mut HashSet<Vec<String>>,
    module_graph: &mut DirectedGraph<Vec<String>>,
) -> Result<(), Diagnostic> {
    let imports = ast.imports.clone();
    for import in imports {
        if import.first().is_some_and(|root| root == "std") {
            continue;
        }
        let Some((source_root, module_path)) = resolve_imported_module(
            importer_path,
            &import,
            local_source_root,
            local_import_root,
            external_modules,
        )?
        else {
            continue;
        };
        let Some(source_path) = module_source_path(source_root, &module_path) else {
            return Err(Diagnostic::new(
                "E0903",
                format!("could not find module `{}`", import.join(".")),
                importer_path,
                1,
                1,
                import.join(".").len().max(1),
                import.join("."),
            ));
        };
        let source_override = module_source_overrides
            .iter()
            .find(|(path, _)| path == &source_path)
            .map(|(_, source)| source.as_str());
        let source = match source_override {
            Some(source) => source.to_string(),
            None => fs::read_to_string(&source_path).map_err(|err| {
                Diagnostic::new(
                    "E0902",
                    format!("failed to read module `{}`: {err}", source_path.display()),
                    importer_path,
                    1,
                    1,
                    1,
                    "",
                )
            })?,
        };
        let tokens = lexer::lex(&source_path, &source)?;
        let mut module_ast = parser::parse(&source_path, &tokens)?;
        reject_script_body(
            &source_path,
            &module_ast,
            "imported modules cannot contain top-level script statements",
        )?;
        if module_ast.package != import {
            return Err(Diagnostic::new(
                "E0904",
                format!(
                    "module `{}` declares package `{}`",
                    import.join("."),
                    module_ast.package.join(".")
                ),
                &source_path,
                1,
                1,
                module_ast.package.join(".").len().max(1),
                module_ast.package.join("."),
            ));
        }
        module_graph.add_edge(ast.package.clone(), module_ast.package.clone());
        if let Some(cycle) = module_graph.find_cycle() {
            let cycle_path = cycle
                .path()
                .iter()
                .map(|package| package.join("."))
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diagnostic::new(
                "E0607",
                format!("cyclic module import: {cycle_path}"),
                importer_path,
                1,
                1,
                import.join(".").len().max(1),
                import.join("."),
            ));
        }
        if !visited.insert(module_ast.package.clone()) {
            continue;
        }
        merge_imported_public_api(
            &source_path,
            &mut module_ast,
            local_source_root,
            local_import_root,
            external_modules,
            module_source_overrides,
            visited,
            module_graph,
        )?;
        merge_public_items(ast, module_ast);
    }
    Ok(())
}

fn resolve_imported_module<'a>(
    importer_path: &Path,
    import: &[String],
    local_source_root: Option<&'a Path>,
    local_import_root: Option<&str>,
    external_modules: &'a [ExternalModule],
) -> Result<Option<(&'a Path, Vec<String>)>, Diagnostic> {
    let Some(import_root) = import.first() else {
        return Ok(None);
    };
    if local_import_root.is_some_and(|root| root == import_root) {
        let Some(source_root) = local_source_root else {
            return Ok(None);
        };
        return Ok(Some((source_root, import[1..].to_vec())));
    }
    if let Some(module) = external_modules
        .iter()
        .find(|module| module.import_root == *import_root)
    {
        return Ok(Some((module.source_root.as_path(), import[1..].to_vec())));
    }
    if external_modules
        .iter()
        .any(|module| module.import_root == *import_root)
    {
        return Ok(None);
    }
    let _ = importer_path;
    Ok(None)
}

fn module_source_path(source_root: &Path, module_path: &[String]) -> Option<PathBuf> {
    if module_path.is_empty() || (module_path.len() == 1 && module_path[0] == "main") {
        let main = source_root.join("main.nomo");
        return main.is_file().then_some(main);
    }
    let mut flat = source_root.to_path_buf();
    for segment in module_path {
        flat.push(segment);
    }
    flat.set_extension("nomo");
    if flat.is_file() {
        return Some(flat);
    }
    let mut dir_main = source_root.to_path_buf();
    for segment in module_path {
        dir_main.push(segment);
    }
    dir_main.push("main.nomo");
    dir_main.is_file().then_some(dir_main)
}

fn merge_public_items(ast: &mut SourceFile, module_ast: SourceFile) {
    let public_structs = module_ast
        .structs
        .iter()
        .filter(|item| item.public)
        .map(|item| item.name.clone())
        .collect::<HashSet<_>>();

    ast.imports.extend(module_ast.imports);
    ast.structs
        .extend(module_ast.structs.into_iter().filter(|item| item.public));
    ast.enums
        .extend(module_ast.enums.into_iter().filter(|item| item.public));
    ast.interfaces
        .extend(module_ast.interfaces.into_iter().filter(|item| item.public));
    ast.consts
        .extend(module_ast.consts.into_iter().filter(|item| item.public));
    ast.extern_blocks.extend(module_ast.extern_blocks);
    ast.functions.extend(
        module_ast
            .functions
            .into_iter()
            .filter(|item| item.public && item.name != "main"),
    );
    ast.impls
        .extend(module_ast.impls.into_iter().filter_map(|mut item| {
            let target = item.type_name.path.first()?;
            if !public_structs.contains(target) {
                return None;
            }
            item.methods.retain(|method| method.public);
            if item.methods.is_empty() {
                None
            } else {
                Some(item)
            }
        }));
}
