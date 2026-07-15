use nomo_syntax::lexer::{Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TopLevelItem {
    Struct(usize),
    Enum(usize),
    Interface(usize),
    ExternOpaque(usize),
    ExternBlock(usize),
    Impl(usize),
    Const(usize),
    Function(usize),
}

pub(super) fn top_level_items(tokens: &[Token]) -> Vec<TopLevelItem> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut structs = 0usize;
    let mut enums = 0usize;
    let mut interfaces = 0usize;
    let mut extern_opaque_types = 0usize;
    let mut extern_blocks = 0usize;
    let mut impls = 0usize;
    let mut consts = 0usize;
    let mut functions = 0usize;
    let mut index = 0usize;

    while let Some(token) = tokens.get(index) {
        if matches!(token.kind, TokenKind::Eof) {
            break;
        }
        if depth == 0 {
            if is_extern_opaque_start(tokens, index) {
                let item = TopLevelItem::ExternOpaque(extern_opaque_types);
                extern_opaque_types += 1;
                items.push(item);
                index += 1;
                continue;
            }
            if matches!(token.kind, TokenKind::Pub) {
                if let Some(item) = public_top_level_item(
                    tokens.get(index + 1),
                    &mut structs,
                    &mut enums,
                    &mut interfaces,
                    &mut consts,
                    &mut functions,
                ) {
                    items.push(item);
                    index += 2;
                    continue;
                }
            } else if let Some(item) = top_level_item(
                &token.kind,
                &mut structs,
                &mut enums,
                &mut interfaces,
                &mut extern_blocks,
                &mut impls,
                &mut consts,
                &mut functions,
            ) {
                items.push(item);
                index += 1;
                continue;
            }
        }

        match token.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }

    items
}

fn is_extern_opaque_start(tokens: &[Token], index: usize) -> bool {
    matches!(
        tokens.get(index).map(|token| &token.kind),
        Some(TokenKind::Extern)
    ) && matches!(
        tokens.get(index + 1).map(|token| &token.kind),
        Some(TokenKind::Ident(name)) if name == "opaque"
    ) && matches!(
        tokens.get(index + 2).map(|token| &token.kind),
        Some(TokenKind::Ident(name)) if name == "type"
    )
}

fn public_top_level_item(
    token: Option<&Token>,
    structs: &mut usize,
    enums: &mut usize,
    interfaces: &mut usize,
    consts: &mut usize,
    functions: &mut usize,
) -> Option<TopLevelItem> {
    match token.map(|token| &token.kind) {
        Some(TokenKind::Struct) => {
            let index = *structs;
            *structs += 1;
            Some(TopLevelItem::Struct(index))
        }
        Some(TokenKind::Enum) => {
            let index = *enums;
            *enums += 1;
            Some(TopLevelItem::Enum(index))
        }
        Some(TokenKind::Interface) => {
            let index = *interfaces;
            *interfaces += 1;
            Some(TopLevelItem::Interface(index))
        }
        Some(TokenKind::Const) => {
            let index = *consts;
            *consts += 1;
            Some(TopLevelItem::Const(index))
        }
        Some(TokenKind::Fn) => {
            let index = *functions;
            *functions += 1;
            Some(TopLevelItem::Function(index))
        }
        _ => None,
    }
}

fn top_level_item(
    kind: &TokenKind,
    structs: &mut usize,
    enums: &mut usize,
    interfaces: &mut usize,
    extern_blocks: &mut usize,
    impls: &mut usize,
    consts: &mut usize,
    functions: &mut usize,
) -> Option<TopLevelItem> {
    match kind {
        TokenKind::Struct => {
            let index = *structs;
            *structs += 1;
            Some(TopLevelItem::Struct(index))
        }
        TokenKind::Enum => {
            let index = *enums;
            *enums += 1;
            Some(TopLevelItem::Enum(index))
        }
        TokenKind::Interface => {
            let index = *interfaces;
            *interfaces += 1;
            Some(TopLevelItem::Interface(index))
        }
        TokenKind::Extern => {
            let index = *extern_blocks;
            *extern_blocks += 1;
            Some(TopLevelItem::ExternBlock(index))
        }
        TokenKind::Impl => {
            let index = *impls;
            *impls += 1;
            Some(TopLevelItem::Impl(index))
        }
        TokenKind::Const => {
            let index = *consts;
            *consts += 1;
            Some(TopLevelItem::Const(index))
        }
        TokenKind::Fn => {
            let index = *functions;
            *functions += 1;
            Some(TopLevelItem::Function(index))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TokenLayout {
    pub(super) package_line: usize,
    pub(super) import_lines: Vec<usize>,
    pub(super) impl_lines: Vec<usize>,
    pub(super) struct_field_lines: Vec<Vec<usize>>,
    pub(super) enum_variant_lines: Vec<Vec<usize>>,
}

impl TokenLayout {
    pub(super) fn from_tokens(tokens: &[Token]) -> Self {
        let package_line = tokens
            .iter()
            .find(|token| matches!(token.kind, TokenKind::Package))
            .map(|token| token.line)
            .unwrap_or(1);
        let import_lines = tokens
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Import))
            .map(|token| token.line)
            .collect::<Vec<_>>();
        let impl_lines = tokens
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::Impl))
            .map(|token| token.line)
            .collect::<Vec<_>>();

        let mut layout = Self {
            package_line,
            import_lines,
            impl_lines,
            struct_field_lines: Vec::new(),
            enum_variant_lines: Vec::new(),
        };
        layout.collect_member_lines(tokens);
        layout
    }

    fn collect_member_lines(&mut self, tokens: &[Token]) {
        let mut index = 0usize;
        let mut depth = 0usize;
        while let Some(token) = tokens.get(index) {
            if matches!(token.kind, TokenKind::Eof) {
                break;
            }
            if depth == 0 {
                let kind_index = if matches!(token.kind, TokenKind::Pub) {
                    index + 1
                } else {
                    index
                };
                match tokens.get(kind_index).map(|token| &token.kind) {
                    Some(TokenKind::Struct) => {
                        let (lines, next_index) = collect_struct_field_lines(tokens, kind_index);
                        self.struct_field_lines.push(lines);
                        index = next_index;
                        continue;
                    }
                    Some(TokenKind::Enum) => {
                        let (lines, next_index) = collect_enum_variant_lines(tokens, kind_index);
                        self.enum_variant_lines.push(lines);
                        index = next_index;
                        continue;
                    }
                    _ => {}
                }
            }
            match token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth = depth.saturating_sub(1),
                _ => {}
            }
            index += 1;
        }
    }
}

fn collect_struct_field_lines(tokens: &[Token], start: usize) -> (Vec<usize>, usize) {
    let Some(open) = find_next_kind(tokens, start, TokenKind::LBrace) else {
        return (Vec::new(), start + 1);
    };
    let mut lines = Vec::new();
    let mut depth = 1usize;
    let mut index = open + 1;
    while let Some(token) = tokens.get(index) {
        match token.kind {
            TokenKind::LBrace => depth += 1,
            TokenKind::RBrace => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return (lines, index + 1);
                }
            }
            TokenKind::Ident(_) if depth == 1 => {
                if matches!(
                    tokens.get(index + 1).map(|token| &token.kind),
                    Some(TokenKind::Colon)
                ) {
                    lines.push(token.line);
                }
            }
            _ => {}
        }
        index += 1;
    }
    (lines, index)
}

fn collect_enum_variant_lines(tokens: &[Token], start: usize) -> (Vec<usize>, usize) {
    let Some(open) = find_next_kind(tokens, start, TokenKind::LBrace) else {
        return (Vec::new(), start + 1);
    };
    let mut lines = Vec::new();
    let mut depth = 1usize;
    let mut index = open + 1;
    while let Some(token) = tokens.get(index) {
        match token.kind {
            TokenKind::LBrace | TokenKind::LParen => depth += 1,
            TokenKind::RBrace | TokenKind::RParen => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return (lines, index + 1);
                }
            }
            TokenKind::Ident(_) if depth == 1 => lines.push(token.line),
            _ => {}
        }
        index += 1;
    }
    (lines, index)
}

fn find_next_kind(tokens: &[Token], start: usize, expected: TokenKind) -> Option<usize> {
    tokens
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, token)| same_token_kind(&token.kind, &expected))
        .map(|(index, _)| index)
}

fn same_token_kind(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}
