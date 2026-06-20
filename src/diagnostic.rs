use std::fmt::Write;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub text: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub text: String,
    pub expected: Option<String>,
    pub found: Option<String>,
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        message: impl Into<String>,
        file: &Path,
        line: usize,
        column: usize,
        length: usize,
        text: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: "error",
            message: message.into(),
            file: file.display().to_string(),
            line,
            column,
            length,
            text: text.into(),
            expected: None,
            found: None,
            suggestions: Vec::new(),
        }
    }

    pub fn with_expected_found(
        mut self,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        self.expected = Some(expected.into());
        self.found = Some(found.into());
        self
    }

    pub fn human(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "error[{}]: {}", self.code, self.message);
        let _ = writeln!(out, "  --> {}:{}:{}", self.file, self.line, self.column);
        let _ = writeln!(out, "   |");
        let _ = writeln!(out, "{:>2} | {}", self.line, self.text);
        let caret_pad = " ".repeat(self.column.saturating_sub(1));
        let carets = "^".repeat(self.length.max(1));
        let _ = writeln!(out, "   | {}{}", caret_pad, carets);
        for suggestion in &self.suggestions {
            let _ = writeln!(out, "help: {}", suggestion.description);
        }
        out
    }

    pub fn json(&self) -> String {
        let suggestions = self
            .suggestions
            .iter()
            .map(|s| {
                format!(
                    "{{\"action\":\"replace_text\",\"range\":{{\"line\":{},\"column\":{},\"length\":{}}},\"text\":\"{}\",\"description\":\"{}\"}}",
                    s.line,
                    s.column,
                    s.length,
                    escape_json(&s.text),
                    escape_json(&s.description)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut type_context = String::new();
        if let Some(expected) = &self.expected {
            let _ = write!(type_context, ",\"expected\":\"{}\"", escape_json(expected));
        }
        if let Some(found) = &self.found {
            let _ = write!(type_context, ",\"found\":\"{}\"", escape_json(found));
        }

        format!(
            "{{\"status\":\"error\",\"error_code\":\"{}\",\"severity\":\"{}\",\"message\":\"{}\",\"source\":{{\"file\":\"{}\",\"line\":{},\"column\":{},\"length\":{},\"text\":\"{}\"}}{},\"suggestions\":[{}]}}",
            self.code,
            self.severity,
            escape_json(&self.message),
            escape_json(&self.file),
            self.line,
            self.column,
            self.length,
            escape_json(&self.text),
            type_context,
            suggestions
        )
    }
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", c as u32);
            }
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escapes_strings() {
        let diagnostic = Diagnostic::new(
            "N0200",
            "bad \"token\"",
            Path::new("main.nomo"),
            1,
            2,
            3,
            "let x = \"a\"",
        );

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"N0200\",\"severity\":\"error\",\"message\":\"bad \\\"token\\\"\",\"source\":{\"file\":\"main.nomo\",\"line\":1,\"column\":2,\"length\":3,\"text\":\"let x = \\\"a\\\"\"},\"suggestions\":[]}"
        );
    }

    #[test]
    fn json_includes_stable_suggestions_shape() {
        let mut diagnostic = Diagnostic::new(
            "N0301",
            "missing import",
            Path::new("src/main.nomo"),
            4,
            5,
            7,
            "println(\"hi\")",
        );
        diagnostic.suggestions.push(Suggestion {
            line: 2,
            column: 1,
            length: 0,
            text: "import std.io.println\n".to_string(),
            description: "add the concrete println import".to_string(),
        });

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"N0301\",\"severity\":\"error\",\"message\":\"missing import\",\"source\":{\"file\":\"src/main.nomo\",\"line\":4,\"column\":5,\"length\":7,\"text\":\"println(\\\"hi\\\")\"},\"suggestions\":[{\"action\":\"replace_text\",\"range\":{\"line\":2,\"column\":1,\"length\":0},\"text\":\"import std.io.println\\n\",\"description\":\"add the concrete println import\"}]}"
        );
    }

    #[test]
    fn json_includes_expected_and_found_when_available() {
        let diagnostic = Diagnostic::new(
            "N0404",
            "type mismatch",
            Path::new("src/main.nomo"),
            3,
            9,
            5,
            "let value: i32 = \"bad\"",
        )
        .with_expected_found("i32", "string");

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"N0404\",\"severity\":\"error\",\"message\":\"type mismatch\",\"source\":{\"file\":\"src/main.nomo\",\"line\":3,\"column\":9,\"length\":5,\"text\":\"let value: i32 = \\\"bad\\\"\"},\"expected\":\"i32\",\"found\":\"string\",\"suggestions\":[]}"
        );
    }

    #[test]
    fn json_diagnostic_sample_snapshot() {
        let diagnostic =
            crate::check_source_text(Path::new("samples/invalid.nomo"), "package main\n@\n")
                .expect_err("source should produce a lexer diagnostic");

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"N0102\",\"severity\":\"error\",\"message\":\"unexpected character `@`\",\"source\":{\"file\":\"samples/invalid.nomo\",\"line\":2,\"column\":1,\"length\":1,\"text\":\"@\"},\"suggestions\":[]}"
        );
    }
}
