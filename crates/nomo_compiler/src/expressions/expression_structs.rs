use super::*;

pub(super) fn lower_struct_literal_value_expr(
    path: &Path,
    type_name: &[String],
    fields: &[(String, AstExpr)],
    scope: &HashMap<String, Binding>,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    expected: Option<&ValueType>,
    span: &Span,
) -> Result<(ValueType, ValueExpr), Diagnostic> {
    let type_name = &type_name[0];
    let Some(struct_type) = structs.get(type_name) else {
        return Err(Diagnostic::new(
            "E0309",
            format!("unknown struct `{type_name}`"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    };
    if is_opaque_handle_struct(struct_type) {
        return Err(Diagnostic::new(
            "E1522",
            format!("opaque handle type `{type_name}` cannot be constructed in Nomo"),
            path,
            span.line,
            span.column,
            span.length,
            &span.text,
        ));
    }
    let struct_args = match expected {
        Some(ValueType::Struct(expected_name, expected_args)) if expected_name == type_name => {
            expected_args.clone()
        }
        _ if struct_type.type_params.is_empty() => Vec::new(),
        _ => {
            return Err(Diagnostic::new(
                "E0317",
                format!("generic struct literal `{type_name}` needs a type annotation"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    };
    let mut seen = HashMap::new();
    for (field_name, _) in fields {
        if seen.insert(field_name.clone(), ()).is_some() {
            return Err(Diagnostic::new(
                "E0311",
                format!("field `{field_name}` is specified more than once"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    }
    let mut lowered_fields = Vec::new();
    for expected_field in &struct_type.fields {
        let expected_field_type = substitute_type_params(
            &expected_field.value_type,
            &struct_type.type_params,
            &struct_args,
        );
        let Some((_, value)) = fields
            .iter()
            .find(|(field_name, _)| field_name == &expected_field.name)
        else {
            return Err(Diagnostic::new(
                "E0310",
                format!(
                    "missing field `{}` for struct `{type_name}`",
                    expected_field.name
                ),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        };
        let (actual_type, lowered) = lower_value_expr_with_expected(
            path,
            value,
            scope,
            imports,
            signatures,
            structs,
            enums,
            Some(&expected_field_type),
            span,
        )?;
        if actual_type != expected_field_type {
            return Err(type_mismatch(
                path,
                span,
                format!(
                    "field `{}` is `{}` but expected `{}`",
                    expected_field.name,
                    actual_type.name(),
                    expected_field_type.name()
                ),
            ));
        }
        lowered_fields.push((expected_field.name.clone(), lowered));
    }
    for (field_name, _) in fields {
        if !struct_type
            .fields
            .iter()
            .any(|field| field.name == *field_name)
        {
            return Err(Diagnostic::new(
                "E0312",
                format!("struct `{type_name}` has no field `{field_name}`"),
                path,
                span.line,
                span.column,
                span.length,
                &span.text,
            ));
        }
    }
    Ok((
        ValueType::Struct(type_name.clone(), struct_args.clone()),
        ValueExpr::StructLiteral {
            type_name: type_name.clone(),
            struct_args,
            fields: lowered_fields,
        },
    ))
}
