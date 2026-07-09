use super::*;

pub(super) fn source_uses_fs_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_fs_builtin(ast)
}

pub(super) fn source_uses_io_read_line(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_io_read_line(ast)
}

pub(super) fn source_uses_env_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_env_builtin(ast)
}

pub(super) fn source_uses_process_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_process_builtin(ast)
}

pub(super) fn source_uses_hash_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_hash_builtin(ast)
}

pub(super) fn source_uses_json_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_json_builtin(ast)
}

pub(super) fn source_uses_regex_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_regex_builtin(ast)
}

pub(super) fn source_uses_num_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_num_builtin(ast)
}

pub(super) fn source_uses_time_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_time_builtin(ast)
}

pub(super) fn source_uses_array_builtin(ast: &SourceFile) -> bool {
    analysis_usage_builtins::source_uses_array_builtin(ast)
}

pub(super) fn source_uses_result_prelude_variant(ast: &SourceFile) -> bool {
    analysis_usage_prelude::source_uses_result_prelude_variant(ast)
}

pub(super) fn source_uses_option_prelude_variant(ast: &SourceFile) -> bool {
    analysis_usage_prelude::source_uses_option_prelude_variant(ast)
}
