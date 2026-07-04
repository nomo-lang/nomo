use std::fmt::Write;
use std::path::Path;

pub const DIAGNOSTIC_DOCS_BASE_URL: &str =
    "https://github.com/nomo-lang/nomo/blob/main/docs/diagnostics";

pub const DOCUMENTED_DIAGNOSTIC_CODES: &[&str] = &[
    "E0101", "E0102", "E0103", "E0104", "E0105", "E0106", "E0107", "E0108", "E0200", "E0201",
    "E0202", "E0203", "E0206", "E0207", "E0208", "E0209", "E0210", "E0211", "E0212", "E0213",
    "E0214", "E0215", "E0216", "E0217", "E0218", "E0219", "E0220", "E0221", "E0222", "E0223",
    "E0224", "E0225", "E0226", "E0227", "E0228", "E0229", "E0230", "E0231", "E0232", "E0233",
    "E0234", "E0235", "E0236", "E0237", "E0238", "E0240", "E0241", "E0242", "E0244", "E0245",
    "E0246", "E0247", "E0248", "E0250", "E0251", "E0252", "E0253", "E0254", "E0255", "E0256",
    "E0257", "E0258", "E0260", "E0261", "E0262", "E0263", "E0264", "E0265", "E0266", "E0267",
    "E0268", "E0269", "E0270", "E0271", "E0272", "E0273", "E0274", "E0300", "E0301", "E0302",
    "E0303", "E0304", "E0305", "E0306", "E0307", "E0308", "E0309", "E0403", "E0404", "E0420",
    "E0421", "E0422", "E0501", "E0901", "E0902", "E0903", "E0904", "E1500", "E1501", "E1502",
    "E1503", "E1504", "E1505", "E1510", "E1511", "E1512", "E1513", "E1514", "E1515", "E1516",
    "E1517", "E1518", "E1519",
];

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

pub fn documented_diagnostic_codes() -> &'static [&'static str] {
    DOCUMENTED_DIAGNOSTIC_CODES
}

pub fn diagnostic_documentation_url(code: &str) -> Option<String> {
    DOCUMENTED_DIAGNOSTIC_CODES
        .binary_search(&code)
        .ok()
        .map(|_| format!("{DIAGNOSTIC_DOCS_BASE_URL}/{code}.md"))
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
            "E0200",
            "bad \"token\"",
            Path::new("main.nomo"),
            1,
            2,
            3,
            "let x = \"a\"",
        );

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"E0200\",\"severity\":\"error\",\"message\":\"bad \\\"token\\\"\",\"source\":{\"file\":\"main.nomo\",\"line\":1,\"column\":2,\"length\":3,\"text\":\"let x = \\\"a\\\"\"},\"suggestions\":[]}"
        );
    }

    #[test]
    fn json_includes_stable_suggestions_shape() {
        let mut diagnostic = Diagnostic::new(
            "E0301",
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
            "{\"status\":\"error\",\"error_code\":\"E0301\",\"severity\":\"error\",\"message\":\"missing import\",\"source\":{\"file\":\"src/main.nomo\",\"line\":4,\"column\":5,\"length\":7,\"text\":\"println(\\\"hi\\\")\"},\"suggestions\":[{\"action\":\"replace_text\",\"range\":{\"line\":2,\"column\":1,\"length\":0},\"text\":\"import std.io.println\\n\",\"description\":\"add the concrete println import\"}]}"
        );
    }

    #[test]
    fn json_includes_expected_and_found_when_available() {
        let diagnostic = Diagnostic::new(
            "E0404",
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
            "{\"status\":\"error\",\"error_code\":\"E0404\",\"severity\":\"error\",\"message\":\"type mismatch\",\"source\":{\"file\":\"src/main.nomo\",\"line\":3,\"column\":9,\"length\":5,\"text\":\"let value: i32 = \\\"bad\\\"\"},\"expected\":\"i32\",\"found\":\"string\",\"suggestions\":[]}"
        );
    }

    #[test]
    fn json_diagnostic_sample_snapshot() {
        let diagnostic =
            crate::check_source_text(Path::new("samples/invalid.nomo"), "package main\n@\n")
                .expect_err("source should produce a lexer diagnostic");

        assert_eq!(
            diagnostic.json(),
            "{\"status\":\"error\",\"error_code\":\"E0102\",\"severity\":\"error\",\"message\":\"unexpected character `@`\",\"source\":{\"file\":\"samples/invalid.nomo\",\"line\":2,\"column\":1,\"length\":1,\"text\":\"@\"},\"suggestions\":[]}"
        );
    }

    #[test]
    fn documented_diagnostic_codes_are_sorted_and_unique() {
        for pair in DOCUMENTED_DIAGNOSTIC_CODES.windows(2) {
            assert!(pair[0] < pair[1], "diagnostic codes must stay sorted");
        }
    }

    #[test]
    fn diagnostic_documentation_url_returns_registered_doc_url() {
        assert_eq!(
            diagnostic_documentation_url("E0102").as_deref(),
            Some("https://github.com/nomo-lang/nomo/blob/main/docs/diagnostics/E0102.md")
        );
        assert_eq!(diagnostic_documentation_url("E9999"), None);
    }
}
