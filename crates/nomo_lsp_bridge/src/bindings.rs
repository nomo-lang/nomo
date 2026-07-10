use nomo_spans::SourceFile;
use nomo_syntax::lexer::{Token, TokenKind};

use super::range::{TextPosition, TextRange, range_contains, token_range_in_file};
use super::{LocalBindingDeclaration, LocalBindingUse};

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalBinding {
    name: String,
    declaration: usize,
    visible_start: usize,
    visible_end: usize,
}

pub(super) fn local_definition_at_position(
    tokens: &[Token],
    source_file: &SourceFile,
    position: TextPosition,
) -> Option<TextRange> {
    let token_index = identifier_token_at_position(tokens, source_file, position)?;
    let bindings = collect_local_bindings(tokens);
    let binding = resolve_local_binding(tokens, &bindings, token_index)?;
    Some(identifier_range(source_file, &tokens[binding.declaration]))
}

pub(super) fn local_references_at_position(
    tokens: &[Token],
    source_file: &SourceFile,
    position: TextPosition,
    include_declaration: bool,
) -> Option<Vec<TextRange>> {
    let token_index = identifier_token_at_position(tokens, source_file, position)?;
    let bindings = collect_local_bindings(tokens);
    let target = resolve_local_binding(tokens, &bindings, token_index)?;
    let references = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            let TokenKind::Ident(name) = &token.kind else {
                return None;
            };
            if name != &target.name {
                return None;
            }
            let resolved = resolve_local_binding(tokens, &bindings, index)?;
            if resolved.declaration != target.declaration
                || (!include_declaration && index == target.declaration)
            {
                return None;
            }
            Some(identifier_range(source_file, token))
        })
        .collect();
    Some(references)
}

pub(super) fn token_resolves_to_local_binding(tokens: &[Token], token_index: usize) -> bool {
    resolve_local_binding(tokens, &collect_local_bindings(tokens), token_index).is_some()
}

pub(super) fn local_binding_token_mask(tokens: &[Token]) -> Vec<bool> {
    let bindings = collect_local_bindings(tokens);
    (0..tokens.len())
        .map(|index| resolve_local_binding(tokens, &bindings, index).is_some())
        .collect()
}

pub(super) fn local_binding_declarations(
    tokens: &[Token],
    source_file: &SourceFile,
) -> Vec<LocalBindingDeclaration> {
    let bindings = collect_local_bindings(tokens);
    let brace_pairs = matching_pairs(tokens, is_lbrace, is_rbrace);
    let paren_pairs = matching_pairs(tokens, is_lparen, is_rparen);
    bindings
        .iter()
        .filter_map(|binding| {
            let (callable_name, callable_owner) =
                enclosing_callable(tokens, &brace_pairs, &paren_pairs, binding.declaration)?;
            Some(LocalBindingDeclaration {
                name: binding.name.clone(),
                range: identifier_range(source_file, &tokens[binding.declaration]),
                callable_name,
                callable_owner,
            })
        })
        .collect()
}

pub(super) fn local_binding_uses(
    tokens: &[Token],
    source_file: &SourceFile,
) -> Vec<LocalBindingUse> {
    let bindings = collect_local_bindings(tokens);
    tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| {
            let binding = resolve_local_binding(tokens, &bindings, index)?;
            Some(LocalBindingUse {
                range: identifier_range(source_file, token),
                declaration: identifier_range(source_file, &tokens[binding.declaration]),
            })
        })
        .collect()
}

pub(super) fn identifier_token_at_position(
    tokens: &[Token],
    source_file: &SourceFile,
    position: TextPosition,
) -> Option<usize> {
    tokens.iter().position(|token| {
        matches!(token.kind, TokenKind::Ident(_))
            && range_contains(identifier_range(source_file, token), position)
    })
}

fn collect_local_bindings(tokens: &[Token]) -> Vec<LocalBinding> {
    let brace_pairs = matching_pairs(tokens, is_lbrace, is_rbrace);
    let paren_pairs = matching_pairs(tokens, is_lparen, is_rparen);
    let mut bindings = Vec::new();

    collect_parameter_bindings(tokens, &brace_pairs, &paren_pairs, &mut bindings);
    collect_let_bindings(tokens, &brace_pairs, &paren_pairs, &mut bindings);
    collect_if_let_bindings(tokens, &brace_pairs, &paren_pairs, &mut bindings);
    collect_for_bindings(tokens, &brace_pairs, &mut bindings);
    collect_match_bindings(tokens, &brace_pairs, &paren_pairs, &mut bindings);

    bindings.sort_by_key(|binding| (binding.declaration, binding.visible_start));
    bindings.dedup_by_key(|binding| binding.declaration);
    bindings
}

fn enclosing_callable(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    paren_pairs: &[Option<usize>],
    declaration: usize,
) -> Option<(String, Option<String>)> {
    let mut selected = None;
    for (fn_index, token) in tokens.iter().enumerate().take(declaration) {
        if !matches!(token.kind, TokenKind::Fn) {
            continue;
        }
        let Some(name_index) = next_significant(tokens, fn_index) else {
            continue;
        };
        let TokenKind::Ident(name) = &tokens[name_index].kind else {
            continue;
        };
        let Some(open_paren) = find_kind(tokens, name_index + 1, declaration, is_lparen) else {
            continue;
        };
        let Some(close_paren) = paren_pairs[open_paren] else {
            continue;
        };
        let Some(body_open) = next_function_body_open(tokens, close_paren) else {
            continue;
        };
        let Some(body_close) = brace_pairs[body_open] else {
            continue;
        };
        if (open_paren < declaration && declaration < close_paren)
            || (body_open < declaration && declaration < body_close)
        {
            selected = Some((
                name.clone(),
                enclosing_impl_owner(tokens, brace_pairs, fn_index),
            ));
        }
    }
    selected
}

fn enclosing_impl_owner(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    fn_index: usize,
) -> Option<String> {
    let mut owner = None;
    for (impl_index, token) in tokens.iter().enumerate().take(fn_index) {
        if !matches!(token.kind, TokenKind::Impl) {
            continue;
        }
        let Some(body_open) = find_kind(tokens, impl_index + 1, fn_index, is_lbrace) else {
            continue;
        };
        let Some(body_close) = brace_pairs[body_open] else {
            continue;
        };
        if fn_index >= body_close {
            continue;
        }
        let target = find_kind(tokens, impl_index + 1, body_open, |kind| {
            matches!(kind, TokenKind::For)
        })
        .and_then(|index| next_significant(tokens, index))
        .or_else(|| next_significant(tokens, impl_index));
        let Some(target) = target else {
            continue;
        };
        if let TokenKind::Ident(name) = &tokens[target].kind {
            owner = Some(name.clone());
        }
    }
    owner
}

fn collect_parameter_bindings(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    paren_pairs: &[Option<usize>],
    bindings: &mut Vec<LocalBinding>,
) {
    for (fn_index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Fn) {
            continue;
        }
        let Some(open_paren) = (fn_index + 1..tokens.len())
            .find(|index| matches!(tokens[*index].kind, TokenKind::LParen))
        else {
            continue;
        };
        let Some(close_paren) = paren_pairs[open_paren] else {
            continue;
        };
        let Some(open_brace) = next_function_body_open(tokens, close_paren) else {
            continue;
        };
        let Some(close_brace) = brace_pairs[open_brace] else {
            continue;
        };

        for segment in comma_separated_segments(tokens, open_paren + 1, close_paren) {
            let Some(declaration) = segment
                .into_iter()
                .find(|index| matches!(tokens[*index].kind, TokenKind::Ident(_)))
            else {
                continue;
            };
            push_binding(bindings, tokens, declaration, open_brace + 1, close_brace);
        }
    }
}

fn collect_let_bindings(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    paren_pairs: &[Option<usize>],
    bindings: &mut Vec<LocalBinding>,
) {
    for (let_index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Let)
            || previous_significant(tokens, let_index)
                .is_some_and(|index| matches!(tokens[index].kind, TokenKind::If))
        {
            continue;
        }
        let Some(head) = next_significant(tokens, let_index) else {
            continue;
        };
        let head = if matches!(tokens[head].kind, TokenKind::Mut) {
            let Some(index) = next_significant(tokens, head) else {
                continue;
            };
            index
        } else {
            head
        };
        if !matches!(tokens[head].kind, TokenKind::Ident(_)) {
            continue;
        }
        let Some(after_head) = next_significant(tokens, head) else {
            continue;
        };
        let scope_end = enclosing_block_end(tokens, brace_pairs, let_index).unwrap_or(tokens.len());

        if matches!(tokens[after_head].kind, TokenKind::LParen) {
            let Some(close_paren) = paren_pairs[after_head] else {
                continue;
            };
            let Some(declaration) = first_identifier(tokens, after_head + 1, close_paren) else {
                continue;
            };
            let Some(else_index) = rfind_kind(tokens, close_paren + 1, scope_end, |kind| {
                matches!(kind, TokenKind::Else)
            }) else {
                continue;
            };
            let Some(else_open) = find_kind(tokens, else_index + 1, scope_end, is_lbrace) else {
                continue;
            };
            let Some(else_close) = brace_pairs[else_open] else {
                continue;
            };
            push_binding(
                bindings,
                tokens,
                declaration,
                else_close.saturating_add(1),
                scope_end,
            );
        } else {
            let visible_start = statement_end(tokens, let_index, scope_end)
                .map_or(head.saturating_add(1), |index| index.saturating_add(1));
            push_binding(bindings, tokens, head, visible_start, scope_end);
        }
    }
}

fn collect_if_let_bindings(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    paren_pairs: &[Option<usize>],
    bindings: &mut Vec<LocalBinding>,
) {
    for (let_index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::Let)
            || !previous_significant(tokens, let_index)
                .is_some_and(|index| matches!(tokens[index].kind, TokenKind::If))
        {
            continue;
        }
        let Some(open_paren) = find_kind(tokens, let_index + 1, tokens.len(), is_lparen) else {
            continue;
        };
        let Some(close_paren) = paren_pairs[open_paren] else {
            continue;
        };
        let Some(declaration) = first_identifier(tokens, open_paren + 1, close_paren) else {
            continue;
        };
        let Some(equal) = find_kind(tokens, close_paren + 1, tokens.len(), |kind| {
            matches!(kind, TokenKind::Equal)
        }) else {
            continue;
        };
        let Some(body_open) = find_kind(tokens, equal + 1, tokens.len(), is_lbrace) else {
            continue;
        };
        let Some(body_close) = brace_pairs[body_open] else {
            continue;
        };
        push_binding(bindings, tokens, declaration, body_open + 1, body_close);
    }
}

fn collect_for_bindings(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    bindings: &mut Vec<LocalBinding>,
) {
    for (for_index, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::For) {
            continue;
        }
        let Some(declaration) = next_significant(tokens, for_index) else {
            continue;
        };
        if !matches!(tokens[declaration].kind, TokenKind::Ident(_))
            || !next_significant(tokens, declaration)
                .is_some_and(|index| matches!(tokens[index].kind, TokenKind::In))
        {
            continue;
        }
        let Some(body_open) = find_kind(tokens, declaration + 1, tokens.len(), is_lbrace) else {
            continue;
        };
        let Some(body_close) = brace_pairs[body_open] else {
            continue;
        };
        push_binding(bindings, tokens, declaration, body_open + 1, body_close);
    }
}

fn collect_match_bindings(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    paren_pairs: &[Option<usize>],
    bindings: &mut Vec<LocalBinding>,
) {
    for (arrow, token) in tokens.iter().enumerate() {
        if !matches!(token.kind, TokenKind::FatArrow) {
            continue;
        }
        let Some(close_paren) = previous_significant(tokens, arrow) else {
            continue;
        };
        if !matches!(tokens[close_paren].kind, TokenKind::RParen) {
            continue;
        }
        let Some(open_paren) = paren_pairs[close_paren] else {
            continue;
        };
        let Some(declaration) = first_identifier(tokens, open_paren + 1, close_paren) else {
            continue;
        };
        let Some(value_start) = next_significant(tokens, arrow) else {
            continue;
        };
        let value_end = if matches!(tokens[value_start].kind, TokenKind::LBrace) {
            brace_pairs[value_start].unwrap_or(tokens.len())
        } else {
            match_arm_expression_end(tokens, value_start)
        };
        push_binding(bindings, tokens, declaration, value_start, value_end);
    }
}

fn resolve_local_binding<'a>(
    tokens: &[Token],
    bindings: &'a [LocalBinding],
    token_index: usize,
) -> Option<&'a LocalBinding> {
    let TokenKind::Ident(name) = &tokens.get(token_index)?.kind else {
        return None;
    };
    bindings
        .iter()
        .filter(|binding| {
            binding.name == *name
                && (binding.declaration == token_index
                    || (token_index >= binding.visible_start
                        && token_index < binding.visible_end
                        && can_resolve_as_local_reference(tokens, token_index)))
        })
        .max_by_key(|binding| {
            (
                binding.declaration == token_index,
                binding.visible_start,
                binding.declaration,
            )
        })
}

fn can_resolve_as_local_reference(tokens: &[Token], index: usize) -> bool {
    if previous_significant(tokens, index)
        .is_some_and(|previous| matches!(tokens[previous].kind, TokenKind::Dot))
    {
        return false;
    }
    !next_significant(tokens, index)
        .is_some_and(|next| matches!(tokens[next].kind, TokenKind::Colon))
}

fn push_binding(
    bindings: &mut Vec<LocalBinding>,
    tokens: &[Token],
    declaration: usize,
    visible_start: usize,
    visible_end: usize,
) {
    let TokenKind::Ident(name) = &tokens[declaration].kind else {
        return;
    };
    bindings.push(LocalBinding {
        name: name.clone(),
        declaration,
        visible_start,
        visible_end,
    });
}

fn matching_pairs(
    tokens: &[Token],
    is_open: fn(&TokenKind) -> bool,
    is_close: fn(&TokenKind) -> bool,
) -> Vec<Option<usize>> {
    let mut pairs = vec![None; tokens.len()];
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if is_open(&token.kind) {
            stack.push(index);
        } else if is_close(&token.kind)
            && let Some(open) = stack.pop()
        {
            pairs[open] = Some(index);
            pairs[index] = Some(open);
        }
    }
    pairs
}

fn comma_separated_segments(tokens: &[Token], start: usize, end: usize) -> Vec<Vec<usize>> {
    let mut segments = vec![Vec::new()];
    let mut nesting = 0usize;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.kind {
            TokenKind::Less | TokenKind::LParen | TokenKind::LBracket => nesting += 1,
            TokenKind::Greater | TokenKind::RParen | TokenKind::RBracket => {
                nesting = nesting.saturating_sub(1);
            }
            TokenKind::Comma if nesting == 0 => segments.push(Vec::new()),
            TokenKind::Newline => {}
            _ => segments.last_mut().expect("one segment exists").push(index),
        }
    }
    segments
}

fn next_function_body_open(tokens: &[Token], close_paren: usize) -> Option<usize> {
    for (index, token) in tokens.iter().enumerate().skip(close_paren + 1) {
        match token.kind {
            TokenKind::LBrace => return Some(index),
            TokenKind::Newline | TokenKind::Eof => return None,
            _ => {}
        }
    }
    None
}

fn enclosing_block_end(
    tokens: &[Token],
    brace_pairs: &[Option<usize>],
    index: usize,
) -> Option<usize> {
    (0..index).rev().find_map(|open| {
        if !matches!(tokens[open].kind, TokenKind::LBrace) {
            return None;
        }
        brace_pairs[open].filter(|close| *close > index)
    })
}

fn statement_end(tokens: &[Token], start: usize, limit: usize) -> Option<usize> {
    let mut parens = 0usize;
    let mut brackets = 0usize;
    let mut braces = 0usize;
    for (index, token) in tokens.iter().enumerate().take(limit).skip(start + 1) {
        match token.kind {
            TokenKind::LParen => parens += 1,
            TokenKind::RParen => parens = parens.saturating_sub(1),
            TokenKind::LBracket => brackets += 1,
            TokenKind::RBracket => brackets = brackets.saturating_sub(1),
            TokenKind::LBrace => braces += 1,
            TokenKind::RBrace => braces = braces.saturating_sub(1),
            TokenKind::Newline if parens == 0 && brackets == 0 && braces == 0 => {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn match_arm_expression_end(tokens: &[Token], start: usize) -> usize {
    let mut braces = 0usize;
    let mut parens = 0usize;
    let mut brackets = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.kind {
            TokenKind::LBrace => braces += 1,
            TokenKind::RBrace if braces == 0 => return index,
            TokenKind::RBrace => braces -= 1,
            TokenKind::LParen => parens += 1,
            TokenKind::RParen => parens = parens.saturating_sub(1),
            TokenKind::LBracket => brackets += 1,
            TokenKind::RBracket => brackets = brackets.saturating_sub(1),
            TokenKind::Newline if braces == 0 && parens == 0 && brackets == 0 => return index,
            _ => {}
        }
    }
    tokens.len()
}

fn first_identifier(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end).find(|index| matches!(tokens[*index].kind, TokenKind::Ident(_)))
}

fn find_kind(
    tokens: &[Token],
    start: usize,
    end: usize,
    predicate: fn(&TokenKind) -> bool,
) -> Option<usize> {
    (start..end.min(tokens.len())).find(|index| predicate(&tokens[*index].kind))
}

fn rfind_kind(
    tokens: &[Token],
    start: usize,
    end: usize,
    predicate: fn(&TokenKind) -> bool,
) -> Option<usize> {
    (start..end.min(tokens.len()))
        .rev()
        .find(|index| predicate(&tokens[*index].kind))
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
        unreachable!("identifier ranges are only requested for identifier tokens")
    };
    token_range_in_file(source_file, token.line, token.column, name)
}

fn is_lbrace(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::LBrace)
}

fn is_rbrace(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::RBrace)
}

fn is_lparen(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::LParen)
}

fn is_rparen(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::RParen)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nomo_spans::SourceMap;
    use nomo_syntax::lexer::lex;
    use std::path::Path;

    fn local_references(source: &str, position: TextPosition) -> Vec<TextRange> {
        let path = Path::new("main.nomo");
        let tokens = lex(path, source).unwrap();
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file(path, source);
        let source_file = source_map.file(file_id).unwrap();
        local_references_at_position(&tokens, source_file, position, true).unwrap()
    }

    #[test]
    fn parameter_references_stay_inside_the_declaring_function() {
        let source = "package app.main\n\nfn first(value: i64) -> i64 {\n    return value\n}\n\nfn second(value: i64) -> i64 {\n    return value\n}\n";

        let references = local_references(
            source,
            TextPosition {
                line: 2,
                character: 9,
            },
        );

        assert_eq!(references.len(), 2);
        assert!(references.iter().all(|range| range.start.line <= 3));
    }

    #[test]
    fn nested_let_binding_shadows_outer_binding() {
        let source = "package app.main\n\nfn main() -> void {\n    let value: i64 = 1\n    for {\n        let value: i64 = 2\n        io.println(value)\n        break\n    }\n    io.println(value)\n}\n";

        let outer = local_references(
            source,
            TextPosition {
                line: 3,
                character: 8,
            },
        );
        let inner = local_references(
            source,
            TextPosition {
                line: 5,
                character: 12,
            },
        );

        assert_eq!(outer.len(), 2);
        assert_eq!(inner.len(), 2);
        assert_eq!(outer[1].start.line, 9);
        assert_eq!(inner[1].start.line, 6);
    }

    #[test]
    fn pattern_bindings_use_their_language_scopes() {
        let source = "package app.main\n\nfn main(values: Array<string>) -> void {\n    let Some(first) = values.get(0) else {\n        panic(\"missing\")\n    }\n    io.println(first)\n    if let Some(second) = values.get(1) {\n        io.println(second)\n    }\n    for value in values {\n        io.println(value)\n    }\n    let label: string = match values.get(0) {\n        Some(text) => text\n        None => \"missing\"\n    }\n}\n";

        for (line, character, use_line) in [(3, 13, 6), (7, 16, 8), (10, 8, 11), (14, 13, 14)] {
            let references = local_references(source, TextPosition { line, character });
            assert_eq!(references.len(), 2, "binding on line {line}");
            assert_eq!(references[1].start.line, use_line);
        }
    }

    #[test]
    fn local_binding_does_not_capture_fields_or_struct_labels() {
        let source = "package app.main\n\nstruct User {\n    value: i64\n}\n\nfn read(value: i64, user: User) -> i64 {\n    let copy: User = User { value: value }\n    user.value\n    return value\n}\n";

        let references = local_references(
            source,
            TextPosition {
                line: 6,
                character: 8,
            },
        );

        assert_eq!(references.len(), 3);
        assert_eq!(
            references
                .iter()
                .map(|range| range.start.line)
                .collect::<Vec<_>>(),
            vec![6, 7, 9]
        );
    }

    #[test]
    fn binding_facts_keep_their_enclosing_function_and_impl_owner() {
        let source = "package app.main\n\nstruct User {\n    name: string\n}\n\nimpl User {\n    fn label(self) -> string {\n        return self.name\n    }\n}\n\nfn read(user: User) -> string {\n    return user.name\n}\n";
        let path = Path::new("main.nomo");
        let tokens = lex(path, source).unwrap();
        let mut source_map = SourceMap::new();
        let file_id = source_map.add_file(path, source);
        let source_file = source_map.file(file_id).unwrap();

        let declarations = local_binding_declarations(&tokens, source_file);
        let uses = local_binding_uses(&tokens, source_file);

        assert_eq!(declarations.len(), 2);
        assert_eq!(declarations[0].name, "self");
        assert_eq!(declarations[0].callable_name, "label");
        assert_eq!(declarations[0].callable_owner.as_deref(), Some("User"));
        assert_eq!(declarations[1].name, "user");
        assert_eq!(declarations[1].callable_name, "read");
        assert_eq!(declarations[1].callable_owner, None);
        assert_eq!(uses.len(), 4);
        assert_eq!(uses[0].declaration, uses[1].declaration);
        assert_eq!(uses[2].declaration, uses[3].declaration);
    }
}
