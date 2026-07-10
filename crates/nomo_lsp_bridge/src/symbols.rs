use nomo_spans::Span;
use nomo_syntax::ast::{EnumDef, ExternBlock, ImplBlock, InterfaceDef, SourceFile, StructDef};
use std::path::Path;

use super::docs::DocComments;
use super::signature::{
    const_signature, enum_signature, extern_function_signature, field_signature,
    function_signature, interface_method_signature, interface_signature, method_signature,
    struct_signature, type_ref, variant_signature,
};
use super::{SemanticSymbol, SemanticSymbolKind, TextPosition, TextRange};

pub(super) fn symbols_from_ast(
    path: &Path,
    ast: &SourceFile,
    docs: &DocComments,
) -> Vec<SemanticSymbol> {
    let mut symbols = Vec::new();
    for item in &ast.structs {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Struct,
            signature: struct_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
        symbols.extend(field_symbols(path, item, docs));
    }
    for item in &ast.enums {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Enum,
            signature: enum_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
        symbols.extend(variant_symbols(path, item, docs));
    }
    for item in &ast.interfaces {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Interface,
            signature: interface_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
        symbols.extend(interface_method_symbols(path, item, docs));
    }
    for item in &ast.consts {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Const,
            signature: const_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.functions {
        symbols.push(SemanticSymbol {
            source_path: path.to_path_buf(),
            name: item.name.clone(),
            kind: SemanticSymbolKind::Function,
            signature: function_signature(item),
            docs: docs
                .item_docs
                .get(&item.span.line)
                .cloned()
                .unwrap_or_default(),
            line: item.span.line,
            range: line_range(&item.span),
            selection_range: name_selection_range(&item.span, &item.name),
        });
    }
    for item in &ast.extern_blocks {
        symbols.extend(extern_function_symbols(path, item, docs));
    }
    for impl_block in &ast.impls {
        symbols.extend(method_symbols(path, impl_block, docs));
    }
    symbols
}

fn field_symbols(path: &Path, item: &StructDef, docs: &DocComments) -> Vec<SemanticSymbol> {
    item.fields
        .iter()
        .map(|field| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: field.name.clone(),
            kind: SemanticSymbolKind::Field,
            signature: field_signature(&item.name, field),
            docs: docs
                .item_docs
                .get(&field.span.line)
                .cloned()
                .unwrap_or_default(),
            line: field.span.line,
            range: line_range(&field.span),
            selection_range: name_selection_range(&field.span, &field.name),
        })
        .collect()
}

fn variant_symbols(path: &Path, item: &EnumDef, docs: &DocComments) -> Vec<SemanticSymbol> {
    item.variants
        .iter()
        .map(|variant| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: variant.name.clone(),
            kind: SemanticSymbolKind::Variant,
            signature: variant_signature(&item.name, variant),
            docs: docs
                .item_docs
                .get(&variant.span.line)
                .cloned()
                .unwrap_or_default(),
            line: variant.span.line,
            range: line_range(&variant.span),
            selection_range: name_selection_range(&variant.span, &variant.name),
        })
        .collect()
}

fn interface_method_symbols(
    path: &Path,
    item: &InterfaceDef,
    docs: &DocComments,
) -> Vec<SemanticSymbol> {
    item.methods
        .iter()
        .map(|method| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: method.name.clone(),
            kind: SemanticSymbolKind::InterfaceMethod,
            signature: interface_method_signature(&item.name, method),
            docs: docs
                .item_docs
                .get(&method.span.line)
                .cloned()
                .unwrap_or_default(),
            line: method.span.line,
            range: line_range(&method.span),
            selection_range: name_selection_range(&method.span, &method.name),
        })
        .collect()
}

fn method_symbols(path: &Path, impl_block: &ImplBlock, docs: &DocComments) -> Vec<SemanticSymbol> {
    let receiver = type_ref(&impl_block.type_name);
    impl_block
        .methods
        .iter()
        .map(|method| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: method.name.clone(),
            kind: SemanticSymbolKind::Method,
            signature: method_signature(&receiver, method),
            docs: docs
                .item_docs
                .get(&method.span.line)
                .cloned()
                .unwrap_or_default(),
            line: method.span.line,
            range: line_range(&method.span),
            selection_range: name_selection_range(&method.span, &method.name),
        })
        .collect()
}

fn extern_function_symbols(
    path: &Path,
    block: &ExternBlock,
    docs: &DocComments,
) -> Vec<SemanticSymbol> {
    block
        .functions
        .iter()
        .map(|function| SemanticSymbol {
            source_path: path.to_path_buf(),
            name: function.name.clone(),
            kind: SemanticSymbolKind::ExternFunction,
            signature: extern_function_signature(&block.abi, function),
            docs: docs
                .item_docs
                .get(&function.span.line)
                .cloned()
                .unwrap_or_default(),
            line: function.span.line,
            range: line_range(&function.span),
            selection_range: name_selection_range(&function.span, &function.name),
        })
        .collect()
}

fn line_range(span: &Span) -> TextRange {
    let line = span.line.saturating_sub(1) as u32;
    TextRange {
        start: TextPosition { line, character: 0 },
        end: TextPosition {
            line,
            character: span.text.chars().map(|ch| ch.len_utf16() as u32).sum(),
        },
    }
}

fn name_selection_range(span: &Span, name: &str) -> TextRange {
    let line = span.line.saturating_sub(1) as u32;
    let fallback_start = span.column.saturating_sub(1) as u32;
    let start = span
        .text
        .find(name)
        .map(|byte_index| span.text[..byte_index].encode_utf16().count() as u32)
        .unwrap_or(fallback_start);
    let end = start + name.encode_utf16().count() as u32;
    TextRange {
        start: TextPosition {
            line,
            character: start,
        },
        end: TextPosition {
            line,
            character: end,
        },
    }
}
