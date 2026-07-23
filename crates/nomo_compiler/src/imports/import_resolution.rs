pub(super) fn resolve_specific_value_builtin(
    name: &str,
    imports: &[String],
) -> Option<Vec<String>> {
    let qualified = match name {
        "format" if imports.iter().any(|item| item == "std.fmt.format") => {
            vec!["fmt".to_string(), "format".to_string()]
        }
        "to_string" if imports.iter().any(|item| item == "std.fmt.to_string") => {
            vec!["fmt".to_string(), "to_string".to_string()]
        }
        "debug_string" if imports.iter().any(|item| item == "std.fmt.debug_string") => {
            vec!["fmt".to_string(), "debug_string".to_string()]
        }
        "len" if imports.iter().any(|item| item == "std.string.len") => {
            vec!["string".to_string(), "len".to_string()]
        }
        "concat" if imports.iter().any(|item| item == "std.string.concat") => {
            vec!["string".to_string(), "concat".to_string()]
        }
        "is_empty" if imports.iter().any(|item| item == "std.string.is_empty") => {
            vec!["string".to_string(), "is_empty".to_string()]
        }
        "contains" if imports.iter().any(|item| item == "std.string.contains") => {
            vec!["string".to_string(), "contains".to_string()]
        }
        "starts_with" if imports.iter().any(|item| item == "std.string.starts_with") => {
            vec!["string".to_string(), "starts_with".to_string()]
        }
        "ends_with" if imports.iter().any(|item| item == "std.string.ends_with") => {
            vec!["string".to_string(), "ends_with".to_string()]
        }
        "split" if imports.iter().any(|item| item == "std.string.split") => {
            vec!["string".to_string(), "split".to_string()]
        }
        "trim" if imports.iter().any(|item| item == "std.string.trim") => {
            vec!["string".to_string(), "trim".to_string()]
        }
        "to_lower" if imports.iter().any(|item| item == "std.string.to_lower") => {
            vec!["string".to_string(), "to_lower".to_string()]
        }
        "to_upper" if imports.iter().any(|item| item == "std.string.to_upper") => {
            vec!["string".to_string(), "to_upper".to_string()]
        }
        "read_to_string" if imports.iter().any(|item| item == "std.fs.read_to_string") => {
            vec!["fs".to_string(), "read_to_string".to_string()]
        }
        "write_string" if imports.iter().any(|item| item == "std.fs.write_string") => {
            vec!["fs".to_string(), "write_string".to_string()]
        }
        "read_bytes" if imports.iter().any(|item| item == "std.fs.read_bytes") => {
            vec!["fs".to_string(), "read_bytes".to_string()]
        }
        "write_bytes" if imports.iter().any(|item| item == "std.fs.write_bytes") => {
            vec!["fs".to_string(), "write_bytes".to_string()]
        }
        "exists" if imports.iter().any(|item| item == "std.fs.exists") => {
            vec!["fs".to_string(), "exists".to_string()]
        }
        "metadata" if imports.iter().any(|item| item == "std.fs.metadata") => {
            vec!["fs".to_string(), "metadata".to_string()]
        }
        "create_dir" if imports.iter().any(|item| item == "std.fs.create_dir") => {
            vec!["fs".to_string(), "create_dir".to_string()]
        }
        "remove_dir" if imports.iter().any(|item| item == "std.fs.remove_dir") => {
            vec!["fs".to_string(), "remove_dir".to_string()]
        }
        "read_dir" if imports.iter().any(|item| item == "std.fs.read_dir") => {
            vec!["fs".to_string(), "read_dir".to_string()]
        }
        "open" if imports.iter().any(|item| item == "std.fs.open") => {
            vec!["fs".to_string(), "open".to_string()]
        }
        "read_line" if imports.iter().any(|item| item == "std.io.read_line") => {
            vec!["io".to_string(), "read_line".to_string()]
        }
        "print" if imports.iter().any(|item| item == "std.debug.print") => {
            vec!["debug".to_string(), "print".to_string()]
        }
        "println" if imports.iter().any(|item| item == "std.debug.println") => {
            vec!["debug".to_string(), "println".to_string()]
        }
        "panic" if imports.iter().any(|item| item == "std.debug.panic") => {
            vec!["debug".to_string(), "panic".to_string()]
        }
        "backtrace" if imports.iter().any(|item| item == "std.debug.backtrace") => {
            vec!["debug".to_string(), "backtrace".to_string()]
        }
        "debug" if imports.iter().any(|item| item == "std.log.debug") => {
            vec!["log".to_string(), "debug".to_string()]
        }
        "info" if imports.iter().any(|item| item == "std.log.info") => {
            vec!["log".to_string(), "info".to_string()]
        }
        "warn" if imports.iter().any(|item| item == "std.log.warn") => {
            vec!["log".to_string(), "warn".to_string()]
        }
        "error" if imports.iter().any(|item| item == "std.log.error") => {
            vec!["log".to_string(), "error".to_string()]
        }
        "enabled" if imports.iter().any(|item| item == "std.log.enabled") => {
            vec!["log".to_string(), "enabled".to_string()]
        }
        "new" if imports.iter().any(|item| item == "std.hash.new") => {
            vec!["hash".to_string(), "new".to_string()]
        }
        "string" if imports.iter().any(|item| item == "std.hash.string") => {
            vec!["hash".to_string(), "string".to_string()]
        }
        "bytes" if imports.iter().any(|item| item == "std.hash.bytes") => {
            vec!["hash".to_string(), "bytes".to_string()]
        }
        "write_string" if imports.iter().any(|item| item == "std.hash.write_string") => {
            vec!["hash".to_string(), "write_string".to_string()]
        }
        "write_bytes" if imports.iter().any(|item| item == "std.hash.write_bytes") => {
            vec!["hash".to_string(), "write_bytes".to_string()]
        }
        "finish" if imports.iter().any(|item| item == "std.hash.finish") => {
            vec!["hash".to_string(), "finish".to_string()]
        }
        "sha256" if imports.iter().any(|item| item == "std.crypto.sha256") => {
            vec!["crypto".to_string(), "sha256".to_string()]
        }
        "sha512" if imports.iter().any(|item| item == "std.crypto.sha512") => {
            vec!["crypto".to_string(), "sha512".to_string()]
        }
        "random_bytes" if imports.iter().any(|item| item == "std.crypto.random_bytes") => {
            vec!["crypto".to_string(), "random_bytes".to_string()]
        }
        "parse" if imports.iter().any(|item| item == "std.json.parse") => {
            vec!["json".to_string(), "parse".to_string()]
        }
        "stringify" if imports.iter().any(|item| item == "std.json.stringify") => {
            vec!["json".to_string(), "stringify".to_string()]
        }
        "get" if imports.iter().any(|item| item == "std.http.get") => {
            vec!["http".to_string(), "get".to_string()]
        }
        "post" if imports.iter().any(|item| item == "std.http.post") => {
            vec!["http".to_string(), "post".to_string()]
        }
        "listen" if imports.iter().any(|item| item == "std.http.listen") => {
            vec!["http".to_string(), "listen".to_string()]
        }
        "accept" if imports.iter().any(|item| item == "std.http.accept") => {
            vec!["http".to_string(), "accept".to_string()]
        }
        "respond_string" if imports.iter().any(|item| item == "std.http.respond_string") => {
            vec!["http".to_string(), "respond_string".to_string()]
        }
        "close_server" if imports.iter().any(|item| item == "std.http.close_server") => {
            vec!["http".to_string(), "close_server".to_string()]
        }
        "close_exchange" if imports.iter().any(|item| item == "std.http.close_exchange") => {
            vec!["http".to_string(), "close_exchange".to_string()]
        }
        "connect" if imports.iter().any(|item| item == "std.net.connect") => {
            vec!["net".to_string(), "connect".to_string()]
        }
        "listen" if imports.iter().any(|item| item == "std.net.listen") => {
            vec!["net".to_string(), "listen".to_string()]
        }
        "udp_bind" if imports.iter().any(|item| item == "std.net.udp_bind") => {
            vec!["net".to_string(), "udp_bind".to_string()]
        }
        "compile" if imports.iter().any(|item| item == "std.regex.compile") => {
            vec!["regex".to_string(), "compile".to_string()]
        }
        "is_match" if imports.iter().any(|item| item == "std.regex.is_match") => {
            vec!["regex".to_string(), "is_match".to_string()]
        }
        "captures" if imports.iter().any(|item| item == "std.regex.captures") => {
            vec!["regex".to_string(), "captures".to_string()]
        }
        "map_new" if imports.iter().any(|item| item == "std.collections.map_new") => {
            vec!["collections".to_string(), "map_new".to_string()]
        }
        "map_len" if imports.iter().any(|item| item == "std.collections.map_len") => {
            vec!["collections".to_string(), "map_len".to_string()]
        }
        "map_get" if imports.iter().any(|item| item == "std.collections.map_get") => {
            vec!["collections".to_string(), "map_get".to_string()]
        }
        "map_contains"
            if imports
                .iter()
                .any(|item| item == "std.collections.map_contains") =>
        {
            vec!["collections".to_string(), "map_contains".to_string()]
        }
        "map_set" if imports.iter().any(|item| item == "std.collections.map_set") => {
            vec!["collections".to_string(), "map_set".to_string()]
        }
        "map_remove"
            if imports
                .iter()
                .any(|item| item == "std.collections.map_remove") =>
        {
            vec!["collections".to_string(), "map_remove".to_string()]
        }
        "set_new" if imports.iter().any(|item| item == "std.collections.set_new") => {
            vec!["collections".to_string(), "set_new".to_string()]
        }
        "set_len" if imports.iter().any(|item| item == "std.collections.set_len") => {
            vec!["collections".to_string(), "set_len".to_string()]
        }
        "set_contains"
            if imports
                .iter()
                .any(|item| item == "std.collections.set_contains") =>
        {
            vec!["collections".to_string(), "set_contains".to_string()]
        }
        "set_insert"
            if imports
                .iter()
                .any(|item| item == "std.collections.set_insert") =>
        {
            vec!["collections".to_string(), "set_insert".to_string()]
        }
        "set_remove"
            if imports
                .iter()
                .any(|item| item == "std.collections.set_remove") =>
        {
            vec!["collections".to_string(), "set_remove".to_string()]
        }
        "get" if imports.iter().any(|item| item == "std.env.get") => {
            vec!["env".to_string(), "get".to_string()]
        }
        "set" if imports.iter().any(|item| item == "std.env.set") => {
            vec!["env".to_string(), "set".to_string()]
        }
        "cwd" if imports.iter().any(|item| item == "std.env.cwd") => {
            vec!["env".to_string(), "cwd".to_string()]
        }
        "home_dir" if imports.iter().any(|item| item == "std.env.home_dir") => {
            vec!["env".to_string(), "home_dir".to_string()]
        }
        "temp_dir" if imports.iter().any(|item| item == "std.env.temp_dir") => {
            vec!["env".to_string(), "temp_dir".to_string()]
        }
        "args" if imports.iter().any(|item| item == "std.env.args") => {
            vec!["env".to_string(), "args".to_string()]
        }
        "exit" if imports.iter().any(|item| item == "std.process.exit") => {
            vec!["process".to_string(), "exit".to_string()]
        }
        "spawn" if imports.iter().any(|item| item == "std.process.spawn") => {
            vec!["process".to_string(), "spawn".to_string()]
        }
        "status" if imports.iter().any(|item| item == "std.process.status") => {
            vec!["process".to_string(), "status".to_string()]
        }
        "exec" if imports.iter().any(|item| item == "std.process.exec") => {
            vec!["process".to_string(), "exec".to_string()]
        }
        "output" if imports.iter().any(|item| item == "std.process.output") => {
            vec!["process".to_string(), "output".to_string()]
        }
        "assert" if imports.iter().any(|item| item == "std.testing.assert") => {
            vec!["testing".to_string(), "assert".to_string()]
        }
        "assert_equal"
            if imports
                .iter()
                .any(|item| item == "std.testing.assert_equal") =>
        {
            vec!["testing".to_string(), "assert_equal".to_string()]
        }
        "assert_error"
            if imports
                .iter()
                .any(|item| item == "std.testing.assert_error") =>
        {
            vec!["testing".to_string(), "assert_error".to_string()]
        }
        "join" if imports.iter().any(|item| item == "std.path.join") => {
            vec!["path".to_string(), "join".to_string()]
        }
        "basename" if imports.iter().any(|item| item == "std.path.basename") => {
            vec!["path".to_string(), "basename".to_string()]
        }
        "dirname" if imports.iter().any(|item| item == "std.path.dirname") => {
            vec!["path".to_string(), "dirname".to_string()]
        }
        "extension" if imports.iter().any(|item| item == "std.path.extension") => {
            vec!["path".to_string(), "extension".to_string()]
        }
        "normalize" if imports.iter().any(|item| item == "std.path.normalize") => {
            vec!["path".to_string(), "normalize".to_string()]
        }
        "is_absolute" if imports.iter().any(|item| item == "std.path.is_absolute") => {
            vec!["path".to_string(), "is_absolute".to_string()]
        }
        "abs" if imports.iter().any(|item| item == "std.math.abs") => {
            vec!["math".to_string(), "abs".to_string()]
        }
        "min" if imports.iter().any(|item| item == "std.math.min") => {
            vec!["math".to_string(), "min".to_string()]
        }
        "max" if imports.iter().any(|item| item == "std.math.max") => {
            vec!["math".to_string(), "max".to_string()]
        }
        "floor" if imports.iter().any(|item| item == "std.math.floor") => {
            vec!["math".to_string(), "floor".to_string()]
        }
        "ceil" if imports.iter().any(|item| item == "std.math.ceil") => {
            vec!["math".to_string(), "ceil".to_string()]
        }
        "round" if imports.iter().any(|item| item == "std.math.round") => {
            vec!["math".to_string(), "round".to_string()]
        }
        "sqrt" if imports.iter().any(|item| item == "std.math.sqrt") => {
            vec!["math".to_string(), "sqrt".to_string()]
        }
        "pow" if imports.iter().any(|item| item == "std.math.pow") => {
            vec!["math".to_string(), "pow".to_string()]
        }
        "sin" if imports.iter().any(|item| item == "std.math.sin") => {
            vec!["math".to_string(), "sin".to_string()]
        }
        "cos" if imports.iter().any(|item| item == "std.math.cos") => {
            vec!["math".to_string(), "cos".to_string()]
        }
        "is_digit" if imports.iter().any(|item| item == "std.char.is_digit") => {
            vec!["char".to_string(), "is_digit".to_string()]
        }
        "is_alpha" if imports.iter().any(|item| item == "std.char.is_alpha") => {
            vec!["char".to_string(), "is_alpha".to_string()]
        }
        "is_whitespace" if imports.iter().any(|item| item == "std.char.is_whitespace") => {
            vec!["char".to_string(), "is_whitespace".to_string()]
        }
        "to_string" if imports.iter().any(|item| item == "std.char.to_string") => {
            vec!["char".to_string(), "to_string".to_string()]
        }
        "platform" if imports.iter().any(|item| item == "std.os.platform") => {
            vec!["os".to_string(), "platform".to_string()]
        }
        "arch" if imports.iter().any(|item| item == "std.os.arch") => {
            vec!["os".to_string(), "arch".to_string()]
        }
        "path_separator" if imports.iter().any(|item| item == "std.os.path_separator") => {
            vec!["os".to_string(), "path_separator".to_string()]
        }
        "line_ending" if imports.iter().any(|item| item == "std.os.line_ending") => {
            vec!["os".to_string(), "line_ending".to_string()]
        }
        "now_millis" if imports.iter().any(|item| item == "std.time.now_millis") => {
            vec!["time".to_string(), "now_millis".to_string()]
        }
        "monotonic_millis"
            if imports
                .iter()
                .any(|item| item == "std.time.monotonic_millis") =>
        {
            vec!["time".to_string(), "monotonic_millis".to_string()]
        }
        "duration_millis"
            if imports
                .iter()
                .any(|item| item == "std.time.duration_millis") =>
        {
            vec!["time".to_string(), "duration_millis".to_string()]
        }
        "duration_seconds"
            if imports
                .iter()
                .any(|item| item == "std.time.duration_seconds") =>
        {
            vec!["time".to_string(), "duration_seconds".to_string()]
        }
        "duration_as_millis"
            if imports
                .iter()
                .any(|item| item == "std.time.duration_as_millis") =>
        {
            vec!["time".to_string(), "duration_as_millis".to_string()]
        }
        "format_duration"
            if imports
                .iter()
                .any(|item| item == "std.time.format_duration") =>
        {
            vec!["time".to_string(), "format_duration".to_string()]
        }
        "sleep" if imports.iter().any(|item| item == "std.time.sleep") => {
            vec!["time".to_string(), "sleep".to_string()]
        }
        "sleep_millis" if imports.iter().any(|item| item == "std.time.sleep_millis") => {
            vec!["time".to_string(), "sleep_millis".to_string()]
        }
        "parse_i64" if imports.iter().any(|item| item == "std.num.parse_i64") => {
            vec!["num".to_string(), "parse_i64".to_string()]
        }
        "parse_u64" if imports.iter().any(|item| item == "std.num.parse_u64") => {
            vec!["num".to_string(), "parse_u64".to_string()]
        }
        "parse_f64" if imports.iter().any(|item| item == "std.num.parse_f64") => {
            vec!["num".to_string(), "parse_f64".to_string()]
        }
        "checked_add" if imports.iter().any(|item| item == "std.num.checked_add") => {
            vec!["num".to_string(), "checked_add".to_string()]
        }
        "checked_sub" if imports.iter().any(|item| item == "std.num.checked_sub") => {
            vec!["num".to_string(), "checked_sub".to_string()]
        }
        "checked_mul" if imports.iter().any(|item| item == "std.num.checked_mul") => {
            vec!["num".to_string(), "checked_mul".to_string()]
        }
        "wrapping_add" if imports.iter().any(|item| item == "std.num.wrapping_add") => {
            vec!["num".to_string(), "wrapping_add".to_string()]
        }
        "wrapping_sub" if imports.iter().any(|item| item == "std.num.wrapping_sub") => {
            vec!["num".to_string(), "wrapping_sub".to_string()]
        }
        "wrapping_mul" if imports.iter().any(|item| item == "std.num.wrapping_mul") => {
            vec!["num".to_string(), "wrapping_mul".to_string()]
        }
        "is_ok" if imports.iter().any(|item| item == "std.result.is_ok") => {
            vec!["result".to_string(), "is_ok".to_string()]
        }
        "is_err" if imports.iter().any(|item| item == "std.result.is_err") => {
            vec!["result".to_string(), "is_err".to_string()]
        }
        "map_err" if imports.iter().any(|item| item == "std.result.map_err") => {
            vec!["result".to_string(), "map_err".to_string()]
        }
        "is_some" if imports.iter().any(|item| item == "std.option.is_some") => {
            vec!["option".to_string(), "is_some".to_string()]
        }
        "is_none" if imports.iter().any(|item| item == "std.option.is_none") => {
            vec!["option".to_string(), "is_none".to_string()]
        }
        "unwrap_or" if imports.iter().any(|item| item == "std.option.unwrap_or") => {
            vec!["option".to_string(), "unwrap_or".to_string()]
        }
        "map" if imports.iter().any(|item| item == "std.option.map") => {
            vec!["option".to_string(), "map".to_string()]
        }
        "and_then" if imports.iter().any(|item| item == "std.option.and_then") => {
            vec!["option".to_string(), "and_then".to_string()]
        }
        "unwrap_or" if imports.iter().any(|item| item == "std.result.unwrap_or") => {
            vec!["result".to_string(), "unwrap_or".to_string()]
        }
        "map" if imports.iter().any(|item| item == "std.result.map") => {
            vec!["result".to_string(), "map".to_string()]
        }
        "and_then" if imports.iter().any(|item| item == "std.result.and_then") => {
            vec!["result".to_string(), "and_then".to_string()]
        }
        "new" if imports.iter().any(|item| item == "std.array.new") => {
            vec!["Array".to_string(), "new".to_string()]
        }
        _ => return None,
    };
    Some(qualified)
}
