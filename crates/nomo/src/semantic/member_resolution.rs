use crate::compiler::{
    Program, Statement, ValueType, check_module_source_text_with_project_modules_and_overrides,
};
use crate::diagnostic::Diagnostic;
use crate::project::{Project, project_module_context};
use nomo_lsp_bridge::{
    LocalBindingDeclaration, LocalBindingUse, SemanticMemberOwner, TextRange,
    local_binding_declarations_for_text, local_binding_uses_for_text, token_range_in_file,
};
use nomo_spans::{SourceFile, SourceMap};
use nomo_syntax::ast::SourceFile as AstSourceFile;
use nomo_syntax::lexer::{Token, TokenKind, lex};
use nomo_syntax::parser::parse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(super) fn member_owners_for_document(
    project: &Project,
    path: &Path,
    source: &str,
    source_overrides: &[(PathBuf, String)],
) -> Result<Vec<SemanticMemberOwner>, Diagnostic> {
    let context = project_module_context(project).map_err(|message| {
        Diagnostic::new(
            "E0901",
            message,
            &project.root.join("nomo.toml"),
            1,
            1,
            1,
            "",
        )
    })?;
    let program = check_module_source_text_with_project_modules_and_overrides(
        path,
        source,
        Some(&context.local_source_root),
        &context.external_import_roots,
        &context.external_modules,
        source_overrides,
    )?;
    member_owners_from_program(path, source, &program)
}

pub(super) fn member_owners_for_text(
    path: &Path,
    source: &str,
) -> Result<Vec<SemanticMemberOwner>, Diagnostic> {
    let program = check_module_source_text_with_project_modules_and_overrides(
        path,
        source,
        None,
        &[],
        &[],
        &[],
    )?;
    member_owners_from_program(path, source, &program)
}

fn member_owners_from_program(
    path: &Path,
    source: &str,
    program: &Program,
) -> Result<Vec<SemanticMemberOwner>, Diagnostic> {
    let declarations = local_binding_declarations_for_text(path, source)?;
    let uses = local_binding_uses_for_text(path, source)?;
    let binding_types = binding_types(program, &declarations);
    let tokens = lex(path, source)?;
    let ast = parse(path, &tokens)?;
    let interface_binding_owners = interface_binding_owners(&ast, &declarations);
    let mut source_map = SourceMap::new();
    let file_id = source_map.add_file(path, source);
    let source_file = source_map
        .file(file_id)
        .expect("source file was just added to the source map");
    let brace_pairs = matching_braces(&tokens);
    let mut owners = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Ident(_)) {
            continue;
        }
        if previous_significant(&tokens, index)
            .is_some_and(|previous| matches!(tokens[previous].kind, TokenKind::Dot))
        {
            collect_chain_owners(
                &tokens,
                source_file,
                index,
                &uses,
                &binding_types,
                &interface_binding_owners,
                program,
                &mut owners,
            );
        } else if next_significant(&tokens, index)
            .is_some_and(|next| matches!(tokens[next].kind, TokenKind::Colon))
            && let Some(owner) = struct_literal_owner(&tokens, &brace_pairs, index, program)
        {
            owners.push(SemanticMemberOwner {
                range: identifier_range(source_file, token),
                owner,
            });
        }
    }

    owners.sort_by_key(|member| {
        (
            member.range.start.line,
            member.range.start.character,
            member.range.end.line,
            member.range.end.character,
        )
    });
    owners.dedup_by(|left, right| left.range == right.range);
    Ok(owners)
}

fn interface_binding_owners(
    ast: &AstSourceFile,
    declarations: &[LocalBindingDeclaration],
) -> Vec<(TextRange, String)> {
    declarations
        .iter()
        .filter(|declaration| declaration.callable_owner.is_none())
        .filter_map(|declaration| {
            let function = ast
                .functions
                .iter()
                .find(|function| function.name == declaration.callable_name)?;
            let parameter = function
                .params
                .iter()
                .find(|parameter| parameter.name == declaration.name)?;
            let [type_parameter] = parameter.type_ref.path.as_slice() else {
                return None;
            };
            let bound = function
                .type_param_bounds
                .iter()
                .find(|bound| bound.parameter == *type_parameter)?;
            let [interface] = bound.interface.path.as_slice() else {
                return None;
            };
            Some((declaration.range, interface.clone()))
        })
        .collect()
}

fn binding_types(
    program: &Program,
    declarations: &[LocalBindingDeclaration],
) -> Vec<(TextRange, ValueType)> {
    let mut grouped = HashMap::<(String, Option<String>), Vec<&LocalBindingDeclaration>>::new();
    for declaration in declarations {
        grouped
            .entry((
                declaration.callable_name.clone(),
                declaration.callable_owner.clone(),
            ))
            .or_default()
            .push(declaration);
    }

    let mut typed = Vec::new();
    for ((callable_name, callable_owner), declarations) in grouped {
        let lowered_name = callable_owner.as_ref().map_or_else(
            || callable_name.clone(),
            |owner| format!("{owner}_{callable_name}"),
        );
        let Some(function) = program
            .functions
            .iter()
            .find(|function| function.name == lowered_name)
        else {
            continue;
        };
        let mut facts = function
            .params
            .iter()
            .map(|parameter| (parameter.name.clone(), parameter.value_type.clone()))
            .collect::<Vec<_>>();
        collect_statement_bindings(&function.body, program, &mut facts);
        let mut consumed = vec![false; facts.len()];
        for declaration in declarations {
            let Some((index, (_, value_type))) = facts
                .iter()
                .enumerate()
                .find(|(index, (name, _))| !consumed[*index] && name == &declaration.name)
            else {
                continue;
            };
            consumed[index] = true;
            typed.push((declaration.range, value_type.clone()));
        }
    }
    typed
}

fn collect_statement_bindings(
    statements: &[Statement],
    program: &Program,
    bindings: &mut Vec<(String, ValueType)>,
) {
    for statement in statements {
        match statement {
            Statement::Let {
                name, value_type, ..
            }
            | Statement::QuestionLet {
                name, value_type, ..
            } => bindings.push((name.clone(), value_type.clone())),
            Statement::LetIf {
                name,
                value_type,
                body,
                else_body,
                ..
            } => {
                bindings.push((name.clone(), value_type.clone()));
                collect_statement_bindings(body, program, bindings);
                collect_statement_bindings(else_body, program, bindings);
            }
            Statement::LetMatch {
                name,
                value_type,
                enum_name,
                enum_args,
                arms,
                ..
            } => {
                bindings.push((name.clone(), value_type.clone()));
                collect_match_arm_bindings(enum_name, enum_args, arms, program, bindings);
            }
            Statement::LetElse {
                binding,
                value_type,
                else_body,
                ..
            } => {
                bindings.push((binding.clone(), value_type.clone()));
                collect_statement_bindings(else_body, program, bindings);
            }
            Statement::IfLet {
                binding,
                value_type,
                body,
                else_body,
                ..
            } => {
                if let (Some(binding), Some(value_type)) = (binding, value_type) {
                    bindings.push((binding.clone(), value_type.clone()));
                }
                collect_statement_bindings(body, program, bindings);
                if let Some(else_body) = else_body {
                    collect_statement_bindings(else_body, program, bindings);
                }
            }
            Statement::If {
                body, else_body, ..
            } => {
                collect_statement_bindings(body, program, bindings);
                collect_statement_bindings(else_body, program, bindings);
            }
            Statement::Match {
                enum_name,
                enum_args,
                arms,
                ..
            } => collect_match_arm_bindings(enum_name, enum_args, arms, program, bindings),
            Statement::Loop { kind, body } => {
                if let crate::compiler::LoopKind::Iterate {
                    binding,
                    element_type,
                    ..
                } = kind
                {
                    bindings.push((binding.clone(), element_type.clone()));
                }
                collect_statement_bindings(body, program, bindings);
            }
            _ => {}
        }
    }
}

fn collect_match_arm_bindings(
    enum_name: &str,
    enum_args: &[ValueType],
    arms: &[crate::compiler::MatchStatementArm],
    program: &Program,
    bindings: &mut Vec<(String, ValueType)>,
) {
    for arm in arms {
        if let Some(binding) = &arm.binding
            && let Some(value_type) = enum_payload_type(program, enum_name, enum_args, &arm.variant)
        {
            bindings.push((binding.clone(), value_type));
        }
        collect_statement_bindings(&arm.body, program, bindings);
    }
}

fn enum_payload_type(
    program: &Program,
    enum_name: &str,
    enum_args: &[ValueType],
    variant: &str,
) -> Option<ValueType> {
    let enum_type = program.enums.iter().find(|item| item.name == enum_name)?;
    let payload = enum_type
        .variants
        .iter()
        .find(|item| item.name == variant)?
        .payload
        .as_ref()?;
    Some(substitute_type_params(
        payload,
        &enum_type.type_params,
        enum_args,
    ))
}

fn collect_chain_owners(
    tokens: &[Token],
    source_file: &SourceFile,
    member_index: usize,
    uses: &[LocalBindingUse],
    binding_types: &[(TextRange, ValueType)],
    interface_binding_owners: &[(TextRange, String)],
    program: &Program,
    owners: &mut Vec<SemanticMemberOwner>,
) {
    let mut path = vec![member_index];
    let mut current = member_index;
    while let Some(dot) = previous_significant(tokens, current) {
        if !matches!(tokens[dot].kind, TokenKind::Dot) {
            break;
        }
        let Some(receiver) = previous_significant(tokens, dot) else {
            return;
        };
        if !matches!(tokens[receiver].kind, TokenKind::Ident(_)) {
            return;
        }
        path.push(receiver);
        current = receiver;
    }
    path.reverse();
    if path.len() < 2 {
        return;
    }

    let root_range = identifier_range(source_file, &tokens[path[0]]);
    if let Some(binding_use) = uses
        .iter()
        .find(|binding_use| binding_use.range == root_range)
    {
        if let Some((_, owner)) = interface_binding_owners
            .iter()
            .find(|(declaration, _)| *declaration == binding_use.declaration)
        {
            owners.push(SemanticMemberOwner {
                range: identifier_range(source_file, &tokens[path[1]]),
                owner: owner.clone(),
            });
            return;
        }
        let Some((_, value_type)) = binding_types
            .iter()
            .find(|(declaration, _)| *declaration == binding_use.declaration)
        else {
            return;
        };
        let mut current_type = value_type.clone();
        for (position, segment) in path.iter().enumerate().skip(1) {
            let ValueType::Struct(owner, args) = &current_type else {
                break;
            };
            let range = identifier_range(source_file, &tokens[*segment]);
            owners.push(SemanticMemberOwner {
                range,
                owner: owner.clone(),
            });
            let is_call = position + 1 == path.len()
                && next_significant(tokens, *segment)
                    .is_some_and(|next| matches!(tokens[next].kind, TokenKind::LParen));
            if is_call {
                break;
            }
            let TokenKind::Ident(field_name) = &tokens[*segment].kind else {
                break;
            };
            let Some(struct_type) = program.structs.iter().find(|item| item.name == *owner) else {
                break;
            };
            let Some(field_type) = struct_type
                .fields
                .iter()
                .find(|field| field.name == *field_name)
                .map(|field| {
                    substitute_type_params(&field.value_type, &struct_type.type_params, args)
                })
            else {
                break;
            };
            current_type = field_type;
        }
        return;
    }

    let TokenKind::Ident(root_name) = &tokens[path[0]].kind else {
        return;
    };
    if program.enums.iter().any(|item| item.name == *root_name) {
        owners.push(SemanticMemberOwner {
            range: identifier_range(source_file, &tokens[path[1]]),
            owner: root_name.clone(),
        });
    }
}

fn struct_literal_owner(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    field_index: usize,
    program: &Program,
) -> Option<String> {
    let open = (0..field_index).rev().find(|index| {
        matches!(tokens[*index].kind, TokenKind::LBrace)
            && brace_pairs[*index].is_some_and(|close| close > field_index)
    })?;
    let owner_index = previous_significant(tokens, open)?;
    let TokenKind::Ident(owner) = &tokens[owner_index].kind else {
        return None;
    };
    program
        .structs
        .iter()
        .any(|item| item.name == *owner)
        .then(|| owner.clone())
}

fn substitute_type_params(
    value_type: &ValueType,
    type_params: &[String],
    args: &[ValueType],
) -> ValueType {
    match value_type {
        ValueType::TypeParam(name) => type_params
            .iter()
            .position(|parameter| parameter == name)
            .and_then(|index| args.get(index).cloned())
            .unwrap_or_else(|| value_type.clone()),
        ValueType::Struct(name, nested) => ValueType::Struct(
            name.clone(),
            nested
                .iter()
                .map(|value_type| substitute_type_params(value_type, type_params, args))
                .collect(),
        ),
        ValueType::Enum(name, nested) => ValueType::Enum(
            name.clone(),
            nested
                .iter()
                .map(|value_type| substitute_type_params(value_type, type_params, args))
                .collect(),
        ),
        ValueType::Array(element) => {
            ValueType::Array(Box::new(substitute_type_params(element, type_params, args)))
        }
        _ => value_type.clone(),
    }
}

fn matching_braces(tokens: &[Token]) -> Vec<Option<usize>> {
    let mut pairs = vec![None; tokens.len()];
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.kind, TokenKind::LBrace) {
            stack.push(index);
        } else if matches!(token.kind, TokenKind::RBrace)
            && let Some(open) = stack.pop()
        {
            pairs[open] = Some(index);
            pairs[index] = Some(open);
        }
    }
    pairs
}

fn previous_significant(tokens: &[Token], index: usize) -> Option<usize> {
    (0..index)
        .rev()
        .find(|candidate| !matches!(tokens[*candidate].kind, TokenKind::Newline | TokenKind::Eof))
}

fn next_significant(tokens: &[Token], index: usize) -> Option<usize> {
    (index + 1..tokens.len())
        .find(|candidate| !matches!(tokens[*candidate].kind, TokenKind::Newline | TokenKind::Eof))
}

fn identifier_range(source_file: &SourceFile, token: &Token) -> TextRange {
    let TokenKind::Ident(name) = &token.kind else {
        unreachable!("member resolution only requests identifier ranges")
    };
    token_range_in_file(source_file, token.line, token.column, name)
}
