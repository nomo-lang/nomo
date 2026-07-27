use super::*;
use std::fmt::Write as _;

pub(super) fn collect_channel_element_types(program: &Program) -> Vec<ValueType> {
    let mut elements = collect_struct_instances(program)
        .into_iter()
        .filter_map(|(name, args)| {
            if name != "Channel" {
                return None;
            }
            let [element] = args.as_slice() else {
                return None;
            };
            Some(element.clone())
        })
        .collect::<Vec<_>>();
    elements.sort_by_key(c_type_name_part);
    elements.dedup();
    elements
}

pub(super) fn emit_channel_base_helpers(out: &mut String) {
    out.push_str(
        r#"typedef struct nomo_channel_base nomo_channel_base;

struct nomo_channel_base {
    uint64_t references;
    uint8_t closed;
    void (*destroy)(nomo_channel_base *);
};

static nomo_channel_base *nomo_channel_base_from_handle(uint64_t handle) {
    return (nomo_channel_base *)(uintptr_t)handle;
}

static void nomo_channel_retain_handle(uint64_t handle) {
    nomo_channel_base *base = nomo_channel_base_from_handle(handle);
    if (base != NULL) {
        base->references += 1u;
    }
}

static void nomo_channel_release_handle(uint64_t handle) {
    nomo_channel_base *base = nomo_channel_base_from_handle(handle);
    if (base == NULL || base->references == 0u) {
        return;
    }
    base->references -= 1u;
    if (base->references == 0u) {
        base->destroy(base);
    }
}
"#,
    );
}

pub(super) fn emit_channel_instance_helpers(
    out: &mut String,
    element_type: &ValueType,
    emit_async: bool,
) {
    let suffix = c_type_name_part(element_type);
    let element = c_type(element_type);
    let channel_type = ValueType::Struct("Channel".to_string(), vec![element_type.clone()]);
    let channel = c_type(&channel_type);
    let error_type = ValueType::Struct("ChannelError".to_string(), Vec::new());
    let error = c_type(&error_type);
    let send_error_type =
        ValueType::Struct("ChannelSendError".to_string(), vec![element_type.clone()]);
    let send_error = c_type(&send_error_type);
    let new_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![channel_type.clone(), error_type.clone()],
    );
    let new_result = c_type(&new_result_type);
    let send_result_type = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, send_error_type.clone()],
    );
    let send_result = c_type(&send_result_type);
    let try_send_type = ValueType::Enum("ChannelTrySend".to_string(), vec![element_type.clone()]);
    let try_send = c_type(&try_send_type);
    let try_receive_type =
        ValueType::Enum("ChannelTryReceive".to_string(), vec![element_type.clone()]);
    let try_receive = c_type(&try_receive_type);
    let option_type = ValueType::Enum("Option".to_string(), vec![element_type.clone()]);
    let option = c_type(&option_type);
    let control = format!("nomo_channel_control_{suffix}");
    let send_registration = format!("nomo_channel_send_registration_{suffix}");
    let receive_registration = format!("nomo_channel_receive_registration_{suffix}");

    if emit_async {
        writeln!(
            out,
            "typedef struct {send_registration} {send_registration};\n\
             typedef struct {receive_registration} {receive_registration};"
        )
        .unwrap();
    }
    writeln!(out, "typedef struct {control} {control};").unwrap();
    writeln!(out, "struct {control} {{").unwrap();
    out.push_str(
        "    nomo_channel_base base;\n\
             size_t capacity;\n\
             size_t head;\n\
             size_t tail;\n\
             size_t count;\n",
    );
    writeln!(out, "    {element} *buffer;").unwrap();
    if emit_async {
        writeln!(
            out,
            "    nomo_async_context *owner_context;\n\
             uint8_t metrics_registered;\n\
             {send_registration} *send_head;\n\
             {send_registration} *send_tail;\n\
             {receive_registration} *receive_head;\n\
             {receive_registration} *receive_tail;"
        )
        .unwrap();
    }
    out.push_str("};\n\n");

    if emit_async {
        writeln!(
            out,
            "struct {send_registration} {{\n\
                 {send_registration} *next;\n\
                 {control} *control;\n\
                 nomo_async_context *context;\n\
                 void *frame;\n\
                 nomo_async_poll_fn poll;\n\
                 nomo_async_select_token *select_token;\n\
                 uint32_t select_arm;\n\
                 {element} value;\n\
                 uint8_t value_owned;\n\
                 uint8_t registered;\n\
                 uint8_t active;\n\
                 uint8_t ready;\n\
                 uint8_t closed;\n\
             }};\n\n\
             struct {receive_registration} {{\n\
                 {receive_registration} *next;\n\
                 {control} *control;\n\
                 nomo_async_context *context;\n\
                 void *frame;\n\
                 nomo_async_poll_fn poll;\n\
                 nomo_async_select_token *select_token;\n\
                 uint32_t select_arm;\n\
                 {element} value;\n\
                 uint8_t value_owned;\n\
                 uint8_t registered;\n\
                 uint8_t active;\n\
                 uint8_t ready;\n\
                 uint8_t closed;\n\
             }};\n"
        )
        .unwrap();
    }

    writeln!(
        out,
        "static {control} *nomo_channel_control_from_handle_{suffix}(uint64_t handle) {{\n\
             return ({control} *)(uintptr_t)handle;\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "static void nomo_channel_destroy_{suffix}(nomo_channel_base *raw_base) {{\n\
             {control} *control = ({control} *)raw_base;"
    )
    .unwrap();
    if emit_async {
        out.push_str(
            "\n    if (control->owner_context != NULL) {\n\
                 uint64_t buffered = (uint64_t)control->count;\n\
                 if (control->owner_context->live_channel_buffered_elements >= buffered) {\n\
                     control->owner_context->live_channel_buffered_elements -= buffered;\n\
                 } else {\n\
                     control->owner_context->live_channel_buffered_elements = 0u;\n\
                 }\n\
             }",
        );
    }
    writeln!(
        out,
        "\n    for (size_t offset = 0u; offset < control->count; offset += 1u) {{\n\
                 size_t index = (control->head + offset) % control->capacity;"
    )
    .unwrap();
    emit_value_release_in_place(out, element_type, "control->buffer[index]", 2);
    out.push_str(
        "    }\n\
             free(control->buffer);\n\
             control->buffer = NULL;\n\
             free(control);\n\
         }\n\n",
    );

    emit_channel_new_helper(
        out,
        &suffix,
        &element,
        &channel,
        &error,
        &new_result,
        &new_result_type,
        &control,
    );

    if emit_async {
        emit_channel_async_queue_helpers(
            out,
            &suffix,
            element_type,
            &control,
            &send_registration,
            &receive_registration,
        );
    }

    emit_channel_try_send_helper(
        out,
        &suffix,
        element_type,
        &channel,
        &send_error,
        &try_send,
        &try_send_type,
        &control,
        emit_async,
    );
    emit_channel_try_receive_helper(
        out,
        &suffix,
        element_type,
        &channel,
        &try_receive,
        &try_receive_type,
        &control,
        emit_async,
    );
    emit_channel_close_helper(out, &suffix, element_type, &channel, &control, emit_async);

    if emit_async {
        emit_channel_async_operations(
            out,
            &suffix,
            element_type,
            &channel,
            &send_error,
            &send_result,
            &send_result_type,
            &try_send,
            &try_send_type,
            &try_receive,
            &try_receive_type,
            &option,
            &option_type,
            &control,
            &send_registration,
            &receive_registration,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_channel_new_helper(
    out: &mut String,
    suffix: &str,
    element: &str,
    channel: &str,
    error: &str,
    result: &str,
    result_type: &ValueType,
    control: &str,
) {
    let ValueType::Enum(_, result_args) = result_type else {
        unreachable!()
    };
    let ok = c_enum_variant_ident("Result", result_args, "Ok");
    let err = c_enum_variant_ident("Result", result_args, "Err");
    writeln!(
        out,
        "static {result} nomo_channel_new_{suffix}(uint64_t capacity) {{\n\
             {result} result = ({result}){{0}};\n\
             const char *error_code = NULL;\n\
             const char *error_message = NULL;\n\
             if (capacity == 0u) {{\n\
                 error_code = \"invalid_capacity\";\n\
                 error_message = \"channel capacity must be at least one\";\n\
             }} else if (capacity > 65536u\n\
                 || capacity > (uint64_t)(SIZE_MAX / sizeof({element}))\n\
                 || capacity * (uint64_t)sizeof({element}) > 67108864u) {{\n\
                 error_code = \"capacity_limit\";\n\
                 error_message = \"channel capacity exceeds the bounded slot limit\";\n\
             }}\n\
             if (error_code != NULL) {{\n\
                 result.tag = {err};\n\
                 result.payload.nomo_payload_Err = ({error}){{\n\
                     .nomo_member_code = nomo_string_literal(error_code),\n\
                     .nomo_member_message = nomo_string_literal(error_message),\n\
                 }};\n\
                 return result;\n\
             }}\n\
             {control} *control = ({control} *)calloc(1u, sizeof({control}));\n\
             if (control != NULL) {{\n\
                 control->buffer = ({element} *)calloc((size_t)capacity, sizeof({element}));\n\
             }}\n\
             if (control == NULL || control->buffer == NULL) {{\n\
                 free(control == NULL ? NULL : control->buffer);\n\
                 free(control);\n\
                 result.tag = {err};\n\
                 result.payload.nomo_payload_Err = ({error}){{\n\
                     .nomo_member_code = nomo_string_literal(\"allocation\"),\n\
                     .nomo_member_message = nomo_string_literal(\"channel storage allocation failed\"),\n\
                 }};\n\
                 return result;\n\
             }}\n\
             control->base.references = 1u;\n\
             control->base.destroy = nomo_channel_destroy_{suffix};\n\
             control->capacity = (size_t)capacity;\n\
             result.tag = {ok};\n\
             result.payload.nomo_payload_Ok = ({channel}){{\n\
                 .nomo_member_handle = (uint64_t)(uintptr_t)control,\n\
             }};\n\
             return result;\n\
         }}\n"
    )
    .unwrap();
}

fn emit_channel_async_queue_helpers(
    out: &mut String,
    suffix: &str,
    element_type: &ValueType,
    control: &str,
    send_registration: &str,
    receive_registration: &str,
) {
    writeln!(
        out,
        "static void nomo_channel_bind_context_{suffix}(\n\
             {control} *control,\n\
             nomo_async_context *context\n\
         ) {{\n\
             if (control == NULL || context == NULL) {{\n\
                 return;\n\
             }}\n\
             if (control->owner_context == NULL) {{\n\
                 control->owner_context = context;\n\
             }}\n\
             if (control->owner_context == context\n\
                 && control->metrics_registered == 0u) {{\n\
                 control->metrics_registered = 1u;\n\
                 context->channel_constructions += 1u;\n\
                 context->live_channel_buffered_elements += (uint64_t)control->count;\n\
                 if (context->live_channel_buffered_elements\n\
                     > context->peak_live_channel_buffered_elements) {{\n\
                     context->peak_live_channel_buffered_elements =\n\
                         context->live_channel_buffered_elements;\n\
                 }}\n\
             }}\n\
         }}\n\n\
         static void nomo_channel_wake_{suffix}(\n\
             nomo_async_context *context,\n\
             void *frame,\n\
             nomo_async_poll_fn poll\n\
         ) {{\n\
             if (nomo_async_ready_enqueue(context, frame, poll) != 0) {{\n\
                 context->runtime_failed = 1u;\n\
                 return;\n\
             }}\n\
             context->channel_wakeups += 1u;\n\
         }}\n\n\
         static int nomo_channel_receive_claim_{suffix}(\n\
             {receive_registration} *receiver\n\
         ) {{\n\
             return receiver->select_token == NULL\n\
                 || nomo_async_select_claim(\n\
                     receiver->select_token,\n\
                     receiver->select_arm\n\
                 ) != 0;\n\
         }}\n\n\
         static int nomo_channel_send_claim_{suffix}(\n\
             {send_registration} *sender\n\
         ) {{\n\
             return sender->select_token == NULL\n\
                 || nomo_async_select_claim(\n\
                     sender->select_token,\n\
                     sender->select_arm\n\
                 ) != 0;\n\
         }}\n\n\
         static void nomo_channel_send_wake_{suffix}(\n\
             {send_registration} *sender\n\
         ) {{\n\
             if (sender->select_token != NULL) {{\n\
                 nomo_async_select_wake(sender->select_token);\n\
                 if (sender->context != NULL) {{\n\
                     sender->context->channel_wakeups += 1u;\n\
                 }}\n\
                 return;\n\
             }}\n\
             nomo_channel_wake_{suffix}(\n\
                 sender->context,\n\
                 sender->frame,\n\
                 sender->poll\n\
             );\n\
         }}\n\n\
         static void nomo_channel_receive_wake_{suffix}(\n\
             {receive_registration} *receiver\n\
         ) {{\n\
             if (receiver->select_token != NULL) {{\n\
                 nomo_async_select_wake(receiver->select_token);\n\
                 if (receiver->context != NULL) {{\n\
                     receiver->context->channel_wakeups += 1u;\n\
                 }}\n\
                 return;\n\
             }}\n\
             nomo_channel_wake_{suffix}(\n\
                 receiver->context,\n\
                 receiver->frame,\n\
                 receiver->poll\n\
             );\n\
         }}\n\n\
         static void nomo_channel_unlink_send_{suffix}(\n\
             {control} *control,\n\
             {send_registration} *target\n\
         ) {{\n\
             {send_registration} *previous = NULL;\n\
             {send_registration} *current = control->send_head;\n\
             while (current != NULL && current != target) {{\n\
                 previous = current;\n\
                 current = current->next;\n\
             }}\n\
             if (current == NULL) {{\n\
                 return;\n\
             }}\n\
             if (previous == NULL) {{\n\
                 control->send_head = current->next;\n\
             }} else {{\n\
                 previous->next = current->next;\n\
             }}\n\
             if (control->send_tail == current) {{\n\
                 control->send_tail = previous;\n\
             }}\n\
             current->next = NULL;\n\
             current->registered = 0u;\n\
             if (current->context != NULL\n\
                 && current->context->live_channel_send_waiters != 0u) {{\n\
                 current->context->live_channel_send_waiters -= 1u;\n\
             }}\n\
         }}\n\n\
         static void nomo_channel_unlink_receive_{suffix}(\n\
             {control} *control,\n\
             {receive_registration} *target\n\
         ) {{\n\
             {receive_registration} *previous = NULL;\n\
             {receive_registration} *current = control->receive_head;\n\
             while (current != NULL && current != target) {{\n\
                 previous = current;\n\
                 current = current->next;\n\
             }}\n\
             if (current == NULL) {{\n\
                 return;\n\
             }}\n\
             if (previous == NULL) {{\n\
                 control->receive_head = current->next;\n\
             }} else {{\n\
                 previous->next = current->next;\n\
             }}\n\
             if (control->receive_tail == current) {{\n\
                 control->receive_tail = previous;\n\
             }}\n\
             current->next = NULL;\n\
             current->registered = 0u;\n\
             if (current->context != NULL\n\
                 && current->context->live_channel_receive_waiters != 0u) {{\n\
                 current->context->live_channel_receive_waiters -= 1u;\n\
             }}\n\
         }}\n\n\
         static void nomo_channel_promote_sender_{suffix}({control} *control) {{\n\
             if (control->count >= control->capacity) {{\n\
                 return;\n\
             }}\n\
             {send_registration} *sender = control->send_head;\n\
             while (sender != NULL && nomo_channel_send_claim_{suffix}(sender) == 0) {{\n\
                 nomo_channel_unlink_send_{suffix}(control, sender);\n\
                 sender = control->send_head;\n\
             }}\n\
             if (sender == NULL) {{\n\
                 return;\n\
             }}\n\
             control->send_head = sender->next;\n\
             if (control->send_head == NULL) {{\n\
                 control->send_tail = NULL;\n\
             }}\n\
             sender->next = NULL;\n\
             sender->registered = 0u;\n\
             if (sender->context != NULL\n\
                 && sender->context->live_channel_send_waiters != 0u) {{\n\
                 sender->context->live_channel_send_waiters -= 1u;\n\
             }}\n\
             control->buffer[control->tail] = sender->value;\n\
             control->tail = (control->tail + 1u) % control->capacity;\n\
             control->count += 1u;\n\
             sender->value_owned = 0u;\n\
             sender->ready = 1u;\n\
             sender->closed = 0u;\n\
             if (sender->context != NULL) {{\n\
                 sender->context->channel_sends += 1u;\n\
                 sender->context->channel_buffered_sends += 1u;\n\
                 sender->context->live_channel_buffered_elements += 1u;\n\
                 if (sender->context->live_channel_buffered_elements\n\
                     > sender->context->peak_live_channel_buffered_elements) {{\n\
                     sender->context->peak_live_channel_buffered_elements =\n\
                         sender->context->live_channel_buffered_elements;\n\
                 }}\n\
                 nomo_channel_send_wake_{suffix}(sender);\n\
             }}\n\
         }}\n"
    )
    .unwrap();

    let _ = element_type;
}

#[allow(clippy::too_many_arguments)]
fn emit_channel_try_send_helper(
    out: &mut String,
    suffix: &str,
    element_type: &ValueType,
    channel: &str,
    send_error: &str,
    result: &str,
    result_type: &ValueType,
    control: &str,
    emit_async: bool,
) {
    let ValueType::Enum(_, args) = result_type else {
        unreachable!()
    };
    let sent = c_enum_variant_ident("ChannelTrySend", args, "Sent");
    let full = c_enum_variant_ident("ChannelTrySend", args, "Full");
    let closed = c_enum_variant_ident("ChannelTrySend", args, "Closed");
    let failed = c_enum_variant_ident("ChannelTrySend", args, "Failed");
    let element = c_type(element_type);
    writeln!(
        out,
        "static {result} nomo_channel_try_send_{suffix}({channel} channel, {element} value) {{\n\
             {result} result = ({result}){{0}};\n\
             {control} *control = nomo_channel_control_from_handle_{suffix}(\n\
                 channel.nomo_member_handle\n\
             );\n\
             if (control == NULL) {{\n\
                 result.tag = {failed};\n\
                 result.payload.nomo_payload_Failed = ({send_error}){{\n\
                     .nomo_member_error = {{\n\
                         .nomo_member_code = nomo_string_literal(\"runtime_unavailable\"),\n\
                         .nomo_member_message = nomo_string_literal(\"channel runtime is unavailable\"),\n\
                     }},\n\
                     .nomo_member_value = value,\n\
                 }};\n\
                 return result;\n\
             }}\n\
             if (control->base.closed != 0u) {{\n\
                 result.tag = {closed};\n\
                 result.payload.nomo_payload_Closed = value;\n\
                 return result;\n\
             }}"
    )
    .unwrap();
    if emit_async {
        writeln!(
            out,
            "\n    while (control->receive_head != NULL) {{\n\
                 nomo_channel_receive_registration_{suffix} *receiver = control->receive_head;\n\
                 if (nomo_channel_receive_claim_{suffix}(receiver) == 0) {{\n\
                     nomo_channel_unlink_receive_{suffix}(control, receiver);\n\
                     continue;\n\
                 }}\n\
                 control->receive_head = receiver->next;\n\
                 if (control->receive_head == NULL) {{\n\
                     control->receive_tail = NULL;\n\
                 }}\n\
                 receiver->next = NULL;\n\
                 receiver->registered = 0u;\n\
                 if (receiver->context != NULL\n\
                     && receiver->context->live_channel_receive_waiters != 0u) {{\n\
                     receiver->context->live_channel_receive_waiters -= 1u;\n\
                 }}\n\
                 receiver->value = value;\n\
                 receiver->value_owned = 1u;\n\
                 receiver->ready = 1u;\n\
                 receiver->closed = 0u;\n\
                 if (receiver->context != NULL) {{\n\
                     receiver->context->channel_receives += 1u;\n\
                     nomo_channel_receive_wake_{suffix}(receiver);\n\
                 }}\n\
                 if (control->owner_context != NULL) {{\n\
                     control->owner_context->channel_sends += 1u;\n\
                     control->owner_context->channel_direct_handoffs += 1u;\n\
                 }}\n\
                 result.tag = {sent};\n\
                 return result;\n\
             }}"
        )
        .unwrap();
    }
    writeln!(
        out,
        "\n    if (control->count == control->capacity) {{\n\
             result.tag = {full};\n\
             result.payload.nomo_payload_Full = value;\n\
             return result;\n\
         }}\n\
         control->buffer[control->tail] = value;\n\
         control->tail = (control->tail + 1u) % control->capacity;\n\
         control->count += 1u;"
    )
    .unwrap();
    if emit_async {
        out.push_str(
            "\n    if (control->owner_context != NULL) {\n\
             control->owner_context->channel_sends += 1u;\n\
             control->owner_context->channel_buffered_sends += 1u;\n\
             control->owner_context->live_channel_buffered_elements += 1u;\n\
             if (control->owner_context->live_channel_buffered_elements\n\
                 > control->owner_context->peak_live_channel_buffered_elements) {\n\
                 control->owner_context->peak_live_channel_buffered_elements =\n\
                     control->owner_context->live_channel_buffered_elements;\n\
             }\n\
         }",
        );
    }
    writeln!(
        out,
        "\n    result.tag = {sent};\n\
         return result;\n\
     }}\n"
    )
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn emit_channel_try_receive_helper(
    out: &mut String,
    suffix: &str,
    element_type: &ValueType,
    channel: &str,
    result: &str,
    result_type: &ValueType,
    control: &str,
    emit_async: bool,
) {
    let ValueType::Enum(_, args) = result_type else {
        unreachable!()
    };
    let value_tag = c_enum_variant_ident("ChannelTryReceive", args, "Value");
    let empty = c_enum_variant_ident("ChannelTryReceive", args, "Empty");
    let closed = c_enum_variant_ident("ChannelTryReceive", args, "Closed");
    writeln!(
        out,
        "static {result} nomo_channel_try_receive_{suffix}({channel} channel) {{\n\
             {result} result = ({result}){{0}};\n\
             {control} *control = nomo_channel_control_from_handle_{suffix}(\n\
                 channel.nomo_member_handle\n\
             );\n\
             if (control == NULL || (control->base.closed != 0u && control->count == 0u)) {{\n\
                 result.tag = {closed};\n\
                 return result;\n\
             }}\n\
             if (control->count == 0u) {{\n\
                 result.tag = {empty};\n\
                 return result;\n\
             }}\n\
             result.tag = {value_tag};\n\
             result.payload.nomo_payload_Value = control->buffer[control->head];\n\
             memset(&control->buffer[control->head], 0, sizeof({}));\n\
             control->head = (control->head + 1u) % control->capacity;\n\
             control->count -= 1u;",
        c_type(element_type)
    )
    .unwrap();
    if emit_async {
        out.push_str(
            "\n    if (control->owner_context != NULL) {\n\
                 control->owner_context->channel_receives += 1u;\n\
                 control->owner_context->channel_buffered_receives += 1u;\n\
                 if (control->owner_context->live_channel_buffered_elements != 0u) {\n\
                     control->owner_context->live_channel_buffered_elements -= 1u;\n\
                 }\n\
             }\n",
        );
        writeln!(out, "    nomo_channel_promote_sender_{suffix}(control);").unwrap();
    }
    out.push_str("    return result;\n}\n\n");
}

fn emit_channel_close_helper(
    out: &mut String,
    suffix: &str,
    _element_type: &ValueType,
    channel: &str,
    control: &str,
    emit_async: bool,
) {
    writeln!(
        out,
        "static void nomo_channel_close_{suffix}({channel} channel) {{\n\
             {control} *control = nomo_channel_control_from_handle_{suffix}(\n\
                 channel.nomo_member_handle\n\
             );\n\
             if (control == NULL || control->base.closed != 0u) {{\n\
                 return;\n\
             }}\n\
             control->base.closed = 1u;"
    )
    .unwrap();
    if emit_async {
        writeln!(
            out,
            "\n    while (control->send_head != NULL) {{\n\
                 nomo_channel_send_registration_{suffix} *sender = control->send_head;\n\
                 if (nomo_channel_send_claim_{suffix}(sender) == 0) {{\n\
                     nomo_channel_unlink_send_{suffix}(control, sender);\n\
                     continue;\n\
                 }}\n\
                 control->send_head = sender->next;\n\
                 sender->next = NULL;\n\
                 sender->registered = 0u;\n\
                 if (sender->context != NULL\n\
                     && sender->context->live_channel_send_waiters != 0u) {{\n\
                     sender->context->live_channel_send_waiters -= 1u;\n\
                 }}\n\
                 sender->ready = 1u;\n\
                 sender->closed = 1u;\n\
                 nomo_channel_send_wake_{suffix}(sender);\n\
             }}\n\
             control->send_tail = NULL;\n\
             while (control->receive_head != NULL) {{\n\
                 nomo_channel_receive_registration_{suffix} *receiver = control->receive_head;\n\
                 if (nomo_channel_receive_claim_{suffix}(receiver) == 0) {{\n\
                     nomo_channel_unlink_receive_{suffix}(control, receiver);\n\
                     continue;\n\
                 }}\n\
                 control->receive_head = receiver->next;\n\
                 receiver->next = NULL;\n\
                 receiver->registered = 0u;\n\
                 if (receiver->context != NULL\n\
                     && receiver->context->live_channel_receive_waiters != 0u) {{\n\
                     receiver->context->live_channel_receive_waiters -= 1u;\n\
                 }}\n\
                 receiver->ready = 1u;\n\
                 receiver->closed = 1u;\n\
                 nomo_channel_receive_wake_{suffix}(receiver);\n\
             }}\n\
             control->receive_tail = NULL;"
        )
        .unwrap();
        out.push_str(
            "\n    if (control->owner_context != NULL) {\n\
                 control->owner_context->channel_closes += 1u;\n\
             }",
        );
    }
    out.push_str("\n}\n\n");
}

#[allow(clippy::too_many_arguments)]
fn emit_channel_async_operations(
    out: &mut String,
    suffix: &str,
    element_type: &ValueType,
    channel: &str,
    send_error: &str,
    send_result: &str,
    send_result_type: &ValueType,
    try_send: &str,
    try_send_type: &ValueType,
    try_receive: &str,
    try_receive_type: &ValueType,
    option: &str,
    option_type: &ValueType,
    control: &str,
    send_registration: &str,
    receive_registration: &str,
) {
    let element = c_type(element_type);
    let ValueType::Enum(_, send_result_args) = send_result_type else {
        unreachable!()
    };
    let send_ok = c_enum_variant_ident("Result", send_result_args, "Ok");
    let send_err = c_enum_variant_ident("Result", send_result_args, "Err");
    let ValueType::Enum(_, try_send_args) = try_send_type else {
        unreachable!()
    };
    let try_sent = c_enum_variant_ident("ChannelTrySend", try_send_args, "Sent");
    let try_full = c_enum_variant_ident("ChannelTrySend", try_send_args, "Full");
    let try_closed = c_enum_variant_ident("ChannelTrySend", try_send_args, "Closed");
    let try_failed = c_enum_variant_ident("ChannelTrySend", try_send_args, "Failed");
    let ValueType::Enum(_, try_receive_args) = try_receive_type else {
        unreachable!()
    };
    let try_value = c_enum_variant_ident("ChannelTryReceive", try_receive_args, "Value");
    let try_empty = c_enum_variant_ident("ChannelTryReceive", try_receive_args, "Empty");
    let ValueType::Enum(_, option_args) = option_type else {
        unreachable!()
    };
    let some = c_enum_variant_ident("Option", option_args, "Some");
    let none = c_enum_variant_ident("Option", option_args, "None");

    writeln!(
        out,
        "static void nomo_channel_send_result_ok_{suffix}({send_result} *result) {{\n\
             memset(result, 0, sizeof(*result));\n\
             result->tag = {send_ok};\n\
         }}\n\n\
         static void nomo_channel_send_result_error_{suffix}(\n\
             {send_result} *result,\n\
             const char *code,\n\
             const char *message,\n\
             {element} value\n\
         ) {{\n\
             memset(result, 0, sizeof(*result));\n\
             result->tag = {send_err};\n\
             result->payload.nomo_payload_Err = ({send_error}){{\n\
                 .nomo_member_error = {{\n\
                     .nomo_member_code = nomo_string_literal(code),\n\
                     .nomo_member_message = nomo_string_literal(message),\n\
                 }},\n\
                 .nomo_member_value = value,\n\
             }};\n\
         }}\n\n\
         static nomo_async_poll nomo_channel_send_start_{suffix}(\n\
             {send_registration} *registration,\n\
             {channel} channel,\n\
             {element} value,\n\
             nomo_async_context *context,\n\
             {send_result} *result,\n\
             nomo_async_select_token *select_token,\n\
             uint32_t select_arm\n\
         ) {{\n\
             memset(registration, 0, sizeof(*registration));\n\
             {control} *control = nomo_channel_control_from_handle_{suffix}(\n\
                 channel.nomo_member_handle\n\
             );\n\
             nomo_channel_bind_context_{suffix}(control, context);\n\
             {try_send} attempt = nomo_channel_try_send_{suffix}(channel, value);\n\
             if (attempt.tag == {try_sent}) {{\n\
                 nomo_channel_send_result_ok_{suffix}(result);\n\
                 return NOMO_ASYNC_POLL_READY;\n\
             }}\n\
             if (attempt.tag == {try_closed}) {{\n\
                 nomo_channel_send_result_error_{suffix}(\n\
                     result,\n\
                     \"closed\",\n\
                     \"channel is closed\",\n\
                     attempt.payload.nomo_payload_Closed\n\
                 );\n\
                 return NOMO_ASYNC_POLL_READY;\n\
             }}\n\
             if (attempt.tag == {try_failed}) {{\n\
                 memset(result, 0, sizeof(*result));\n\
                 result->tag = {send_err};\n\
                 result->payload.nomo_payload_Err = attempt.payload.nomo_payload_Failed;\n\
                 return NOMO_ASYNC_POLL_READY;\n\
             }}\n\
             if (attempt.tag != {try_full}) {{\n\
                 context->runtime_failed = 1u;\n\
                 nomo_channel_send_result_error_{suffix}(\n\
                     result,\n\
                     \"runtime_unavailable\",\n\
                     \"channel entered an invalid send state\",\n\
                     value\n\
                 );\n\
                 return NOMO_ASYNC_POLL_READY;\n\
             }}\n\
             registration->control = control;\n\
             registration->context = context;\n\
             registration->frame = context->current_frame;\n\
             registration->poll = context->current_poll;\n\
             registration->select_token = select_token;\n\
             registration->select_arm = select_arm;\n\
             registration->value = attempt.payload.nomo_payload_Full;\n\
             registration->value_owned = 1u;\n\
             registration->registered = 1u;\n\
             registration->active = 1u;\n\
             if (control->send_tail == NULL) {{\n\
                 control->send_head = registration;\n\
             }} else {{\n\
                 control->send_tail->next = registration;\n\
             }}\n\
             control->send_tail = registration;\n\
             nomo_channel_retain_handle(channel.nomo_member_handle);\n\
             context->channel_send_suspensions += 1u;\n\
             context->live_channel_send_waiters += 1u;\n\
             if (context->live_channel_send_waiters\n\
                 > context->peak_live_channel_send_waiters) {{\n\
                 context->peak_live_channel_send_waiters =\n\
                     context->live_channel_send_waiters;\n\
             }}\n\
             context->pending_reason = NOMO_ASYNC_PENDING_CHANNEL;\n\
             return NOMO_ASYNC_POLL_PENDING;\n\
         }}\n\n\
         static nomo_async_poll nomo_channel_send_resume_{suffix}(\n\
             {send_registration} *registration,\n\
             nomo_async_context *context,\n\
             {send_result} *result\n\
         ) {{\n\
             if (registration->ready == 0u) {{\n\
                 context->pending_reason = NOMO_ASYNC_PENDING_CHANNEL;\n\
                 return NOMO_ASYNC_POLL_PENDING;\n\
             }}\n\
             if (registration->closed != 0u) {{\n\
                 nomo_channel_send_result_error_{suffix}(\n\
                     result,\n\
                     \"closed\",\n\
                     \"channel is closed\",\n\
                     registration->value\n\
                 );\n\
                 registration->value_owned = 0u;\n\
             }} else {{\n\
                 nomo_channel_send_result_ok_{suffix}(result);\n\
             }}\n\
             if (registration->active != 0u) {{\n\
                 registration->active = 0u;\n\
                 nomo_channel_release_handle(\n\
                     (uint64_t)(uintptr_t)registration->control\n\
                 );\n\
             }}\n\
             registration->select_token = NULL;\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }}\n\n\
         static void nomo_channel_send_cancel_{suffix}(\n\
             {send_registration} *registration\n\
         ) {{\n\
             if (registration->active == 0u) {{\n\
                 return;\n\
             }}\n\
             if (registration->registered != 0u) {{\n\
                 nomo_channel_unlink_send_{suffix}(\n\
                     registration->control,\n\
                     registration\n\
                 );\n\
             }}\n\
             if (registration->value_owned != 0u) {{"
    )
    .unwrap();
    emit_value_release_in_place(out, element_type, "registration->value", 2);
    writeln!(
        out,
        "        registration->value_owned = 0u;\n\
             }}\n\
             registration->active = 0u;\n\
             registration->select_token = NULL;\n\
             if (registration->context != NULL) {{\n\
                 registration->context->channel_cancellations += 1u;\n\
             }}\n\
             nomo_channel_release_handle((uint64_t)(uintptr_t)registration->control);\n\
         }}\n\n\
         static void nomo_channel_send_select_cancel_{suffix}(\n\
             void *raw_registration,\n\
             nomo_async_context *context\n\
         ) {{\n\
             (void)context;\n\
             nomo_channel_send_cancel_{suffix}(\n\
                 ({send_registration} *)raw_registration\n\
             );\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        out,
        "static nomo_async_poll nomo_channel_receive_start_{suffix}(\n\
             {receive_registration} *registration,\n\
             {channel} channel,\n\
             nomo_async_context *context,\n\
             {option} *result,\n\
             nomo_async_select_token *select_token,\n\
             uint32_t select_arm\n\
         ) {{\n\
             memset(registration, 0, sizeof(*registration));\n\
             {control} *control = nomo_channel_control_from_handle_{suffix}(\n\
                 channel.nomo_member_handle\n\
             );\n\
             nomo_channel_bind_context_{suffix}(control, context);\n\
             {try_receive} attempt = nomo_channel_try_receive_{suffix}(channel);\n\
             if (attempt.tag == {try_value}) {{\n\
                 memset(result, 0, sizeof(*result));\n\
                 result->tag = {some};\n\
                 result->payload.nomo_payload_Some = attempt.payload.nomo_payload_Value;\n\
                 return NOMO_ASYNC_POLL_READY;\n\
             }}\n\
             if (attempt.tag != {try_empty}) {{\n\
                 memset(result, 0, sizeof(*result));\n\
                 result->tag = {none};\n\
                 return NOMO_ASYNC_POLL_READY;\n\
             }}\n\
             registration->control = control;\n\
             registration->context = context;\n\
             registration->frame = context->current_frame;\n\
             registration->poll = context->current_poll;\n\
             registration->select_token = select_token;\n\
             registration->select_arm = select_arm;\n\
             registration->registered = 1u;\n\
             registration->active = 1u;\n\
             if (control->receive_tail == NULL) {{\n\
                 control->receive_head = registration;\n\
             }} else {{\n\
                 control->receive_tail->next = registration;\n\
             }}\n\
             control->receive_tail = registration;\n\
             nomo_channel_retain_handle(channel.nomo_member_handle);\n\
             context->channel_receive_suspensions += 1u;\n\
             context->live_channel_receive_waiters += 1u;\n\
             if (context->live_channel_receive_waiters\n\
                 > context->peak_live_channel_receive_waiters) {{\n\
                 context->peak_live_channel_receive_waiters =\n\
                     context->live_channel_receive_waiters;\n\
             }}\n\
             context->pending_reason = NOMO_ASYNC_PENDING_CHANNEL;\n\
             return NOMO_ASYNC_POLL_PENDING;\n\
         }}\n\n\
         static nomo_async_poll nomo_channel_receive_resume_{suffix}(\n\
             {receive_registration} *registration,\n\
             nomo_async_context *context,\n\
             {option} *result\n\
         ) {{\n\
             if (registration->ready == 0u) {{\n\
                 context->pending_reason = NOMO_ASYNC_PENDING_CHANNEL;\n\
                 return NOMO_ASYNC_POLL_PENDING;\n\
             }}\n\
             memset(result, 0, sizeof(*result));\n\
             if (registration->closed != 0u) {{\n\
                 result->tag = {none};\n\
             }} else {{\n\
                 result->tag = {some};\n\
                 result->payload.nomo_payload_Some = registration->value;\n\
                 registration->value_owned = 0u;\n\
             }}\n\
             if (registration->active != 0u) {{\n\
                 registration->active = 0u;\n\
                 nomo_channel_release_handle(\n\
                     (uint64_t)(uintptr_t)registration->control\n\
                 );\n\
             }}\n\
             registration->select_token = NULL;\n\
             return NOMO_ASYNC_POLL_READY;\n\
         }}\n\n\
         static void nomo_channel_receive_cancel_{suffix}(\n\
             {receive_registration} *registration\n\
         ) {{\n\
             if (registration->active == 0u) {{\n\
                 return;\n\
             }}\n\
             if (registration->registered != 0u) {{\n\
                 nomo_channel_unlink_receive_{suffix}(\n\
                     registration->control,\n\
                     registration\n\
                 );\n\
             }}\n\
             if (registration->value_owned != 0u) {{"
    )
    .unwrap();
    emit_value_release_in_place(out, element_type, "registration->value", 2);
    writeln!(
        out,
        "        registration->value_owned = 0u;\n\
             }}\n\
             registration->active = 0u;\n\
             registration->select_token = NULL;\n\
             if (registration->context != NULL) {{\n\
                 registration->context->channel_cancellations += 1u;\n\
             }}\n\
             nomo_channel_release_handle((uint64_t)(uintptr_t)registration->control);\n\
         }}\n\n\
         static void nomo_channel_receive_select_cancel_{suffix}(\n\
             void *raw_registration,\n\
             nomo_async_context *context\n\
         ) {{\n\
             (void)context;\n\
             nomo_channel_receive_cancel_{suffix}(\n\
                 ({receive_registration} *)raw_registration\n\
             );\n\
         }}\n"
    )
    .unwrap();
}
