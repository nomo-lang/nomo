pub use nomo_diagnostics::*;

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn json_diagnostic_sample_snapshot() {
        let diagnostic = crate::lexer::lex(Path::new("samples/invalid.nomo"), "package main\n@\n")
            .expect_err("source should produce a lexer diagnostic");

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"E0102\",\"severity\":\"error\",\"message\":\"unexpected character `@`\",\"source\":{\"file\":\"samples/invalid.nomo\",\"line\":2,\"column\":1,\"length\":1,\"text\":\"@\"},\"suggestions\":[]}"
        );
    }
}
