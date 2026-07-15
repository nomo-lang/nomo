use super::*;

pub(super) fn lower_structs(
    path: &Path,
    structs: &[AstStructDef],
    enums: &[AstEnumDef],
    opaque_types: &[AstExternOpaqueType],
    standard_type_needs: StandardTypeNeeds,
) -> Result<Vec<StructType>, Diagnostic> {
    let mut lowered = Vec::new();
    let mut known = HashMap::new();
    for item in structs {
        if known.contains_key(&item.name) {
            return Err(Diagnostic::new(
                "E0306",
                format!("struct `{}` is already defined", item.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        known.insert(item.name.clone(), item.type_params.len());
    }
    let known_structs = known
        .iter()
        .map(|(name, arity)| (name.clone(), *arity))
        .chain(
            opaque_types
                .iter()
                .map(|item| (item.name.clone(), OPAQUE_HANDLE_ARITY)),
        )
        .chain(standard_struct_names(standard_type_needs))
        .collect::<Vec<_>>();
    let known_enums = enums
        .iter()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .chain(standard_enum_names(standard_type_needs))
        .collect::<Vec<_>>();

    for item in structs {
        let mut fields = Vec::new();
        let mut field_names = HashMap::new();
        for field in &item.fields {
            if field_names.contains_key(&field.name) {
                return Err(Diagnostic::new(
                    "E0307",
                    format!(
                        "field `{}` is already defined on `{}`",
                        field.name, item.name
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
            field_names.insert(field.name.clone(), ());
            let value_type = parse_value_type_with_names(
                &field.type_ref,
                &known_structs,
                &known_enums,
                &item.type_params,
            )
            .ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &synthetic_span(),
                    &field.type_ref,
                    format!(
                        "unsupported field type `{}` in v0.1 current implementation",
                        field.type_ref.path.join(".")
                    ),
                    &known_structs,
                    &known_enums,
                )
            })?;
            ensure_supported_value_type(path, &value_type, &synthetic_span())?;
            if value_type == ValueType::Void {
                return Err(Diagnostic::new(
                    "E0403",
                    "struct fields cannot have type `void`",
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
            fields.push(StructField {
                name: field.name.clone(),
                value_type,
            });
        }
        lowered.push(StructType {
            package: item.package.join("."),
            name: item.name.clone(),
            type_params: item.type_params.clone(),
            fields,
        });
    }

    Ok(lowered)
}

pub(super) fn lower_enums(
    path: &Path,
    structs: &[StructType],
    enums: &[AstEnumDef],
    opaque_types: &[AstExternOpaqueType],
    standard_type_needs: StandardTypeNeeds,
) -> Result<Vec<EnumType>, Diagnostic> {
    let mut lowered = Vec::new();
    let mut known = HashMap::new();
    for item in enums {
        if known.contains_key(&item.name) {
            return Err(Diagnostic::new(
                "E0313",
                format!("enum `{}` is already defined", item.name),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        known.insert(item.name.clone(), ());
        let mut variants = Vec::new();
        let mut variant_names = HashMap::new();
        for variant in &item.variants {
            if variant_names.contains_key(&variant.name) {
                return Err(Diagnostic::new(
                    "E0314",
                    format!(
                        "variant `{}` is already defined on `{}`",
                        variant.name, item.name
                    ),
                    path,
                    1,
                    1,
                    1,
                    "",
                ));
            }
            variant_names.insert(variant.name.clone(), ());
            let payload = if let Some(type_ref) = &variant.payload {
                let type_name = type_ref.path.first().cloned().unwrap_or_default();
                let known_structs = structs
                    .iter()
                    .map(|item| (item.name.clone(), item.type_params.len()))
                    .chain(
                        opaque_types
                            .iter()
                            .map(|item| (item.name.clone(), OPAQUE_HANDLE_ARITY)),
                    )
                    .chain(standard_struct_names(standard_type_needs))
                    .collect::<Vec<_>>();
                let known_enums = enums
                    .iter()
                    .map(|item| (item.name.clone(), item.type_params.len()))
                    .chain(standard_enum_names(standard_type_needs))
                    .collect::<Vec<_>>();
                let payload_type = parse_value_type_with_names(
                    type_ref,
                    &known_structs,
                    &known_enums,
                    &item.type_params,
                )
                .ok_or_else(|| {
                    unsupported_type_diagnostic(
                        path,
                        &synthetic_span(),
                        type_ref,
                        format!(
                            "unsupported enum payload type `{}` in v0.1 current implementation",
                            type_ref.path.join(".")
                        ),
                        &known_structs,
                        &known_enums,
                    )
                })?;
                ensure_supported_value_type(path, &payload_type, &synthetic_span())?;
                if payload_type == ValueType::Void {
                    return Err(Diagnostic::new(
                        "E0403",
                        format!("enum variant `{}` cannot carry `void`", type_name),
                        path,
                        1,
                        1,
                        1,
                        "",
                    ));
                }
                Some(payload_type)
            } else {
                None
            };
            variants.push(EnumVariantType {
                name: variant.name.clone(),
                payload,
            });
        }
        lowered.push(EnumType {
            package: item.package.join("."),
            name: item.name.clone(),
            type_params: item.type_params.clone(),
            variants,
        });
    }
    Ok(lowered)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StandardTypeNeeds {
    pub(super) io: bool,
    pub(super) fs: bool,
    pub(super) env: bool,
    pub(super) process: bool,
    pub(super) net: bool,
    pub(super) http: bool,
    pub(super) hash: bool,
    pub(super) json: bool,
    pub(super) regex: bool,
    pub(super) collections: bool,
    pub(super) time: bool,
    pub(super) num: bool,
    pub(super) result: bool,
    pub(super) option: bool,
    pub(super) array: bool,
    pub(super) ffi: bool,
}

pub(super) fn function_signature(
    path: &Path,
    function: &AstFunction,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<FunctionSignature, Diagnostic> {
    let struct_names = struct_type_names(structs);
    let enum_names = enums
        .values()
        .map(|item| (item.name.clone(), item.type_params.len()))
        .collect::<Vec<_>>();
    let params = function
        .params
        .iter()
        .map(|param| {
            let value_type = parse_value_type_with_names(
                &param.type_ref,
                &struct_names,
                &enum_names,
                &function.type_params,
            )
            .ok_or_else(|| {
                unsupported_type_diagnostic(
                    path,
                    &function.span,
                    &param.type_ref,
                    "unsupported parameter type in v0.1 current implementation",
                    &struct_names,
                    &enum_names,
                )
            })?;
            ensure_supported_value_type(path, &value_type, &synthetic_span())?;
            Ok(ParamSignature {
                value_type,
                mutable: param.mutable,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let return_type = parse_value_type_with_names(
        &function.return_type,
        &struct_names,
        &enum_names,
        &function.type_params,
    )
    .ok_or_else(|| {
        unsupported_type_diagnostic(
            path,
            &function.span,
            &function.return_type,
            format!(
                "unsupported return type `{}` in v0.1 current implementation",
                function.return_type.path.join(".")
            ),
            &struct_names,
            &enum_names,
        )
    })?;
    ensure_supported_value_type(path, &return_type, &synthetic_span())?;
    Ok(FunctionSignature {
        type_params: function.type_params.clone(),
        params,
        return_type,
        extern_symbol: None,
    })
}

pub(super) fn lower_function_as(
    path: &Path,
    function: &AstFunction,
    lowered_name: &str,
    imports: &[String],
    signatures: &HashMap<String, FunctionSignature>,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
    consts: &[(String, ValueType)],
) -> Result<Function, Diagnostic> {
    let signature = signatures
        .get(lowered_name)
        .expect("signature table is built before lowering");
    let mut scope = HashMap::new();
    for (name, value_type) in consts {
        scope.insert(
            name.clone(),
            Binding {
                value_type: value_type.clone(),
                mutable: false,
                source: BindingSource::Local,
            },
        );
    }
    let mut params = Vec::new();
    for (param, value_type) in function.params.iter().zip(signature.params.iter()) {
        if scope.contains_key(&param.name) {
            return Err(Diagnostic::new(
                "E0302",
                format!(
                    "parameter `{}` is already defined in this scope",
                    param.name
                ),
                path,
                1,
                1,
                1,
                "",
            ));
        }
        scope.insert(
            param.name.clone(),
            Binding {
                value_type: value_type.value_type.clone(),
                mutable: param.mutable,
                source: BindingSource::Param,
            },
        );
        params.push(Parameter {
            name: param.name.clone(),
            mutable: param.mutable,
            value_type: value_type.value_type.clone(),
        });
    }

    let mut body = Vec::new();
    for (index, stmt) in function.body.iter().enumerate() {
        let is_tail = index + 1 == function.body.len();
        lower_stmt_into(
            path,
            stmt,
            &mut scope,
            imports,
            signatures,
            structs,
            enums,
            &signature.return_type,
            is_tail,
            0,
            &mut body,
        )?;
    }

    if signature.return_type != ValueType::Void && !statements_satisfy_function_return(&body) {
        return Err(Diagnostic::new(
            "E0406",
            format!(
                "function `{}` must return `{}`",
                function.name,
                signature.return_type.name()
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    }

    Ok(Function {
        package: function.package.join("."),
        name: lowered_name.to_string(),
        params,
        return_type: signature.return_type.clone(),
        body,
    })
}

pub(super) fn validate_method_self(
    path: &Path,
    method: &AstFunction,
    owner_name: &str,
    structs: &HashMap<String, StructType>,
    enums: &HashMap<String, EnumType>,
) -> Result<(), Diagnostic> {
    let Some(self_param) = method.params.first() else {
        return Err(Diagnostic::new(
            "E0256",
            format!("method `{owner_name}.{}` must declare `self`", method.name),
            path,
            1,
            1,
            1,
            "",
        ));
    };
    if self_param.name != "self" {
        return Err(Diagnostic::new(
            "E0256",
            format!(
                "method `{owner_name}.{}` first parameter must be `self`",
                method.name
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    let Some(ValueType::Struct(self_type, self_args)) =
        parse_value_type(&self_param.type_ref, structs, enums)
    else {
        return Err(Diagnostic::new(
            "E0257",
            format!(
                "method `{owner_name}.{}` has invalid `self` type",
                method.name
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    };
    if self_type != owner_name || !self_args.is_empty() {
        return Err(Diagnostic::new(
            "E0257",
            format!(
                "method `{owner_name}.{}` declares `self` as `{self_type}`",
                method.name
            ),
            path,
            1,
            1,
            1,
            "",
        ));
    }
    Ok(())
}
