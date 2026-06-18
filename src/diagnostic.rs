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
            suggestions: Vec::new(),
        }
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

        format!(
            "{{\"status\":\"error\",\"error_code\":\"{}\",\"severity\":\"{}\",\"message\":\"{}\",\"source\":{{\"file\":\"{}\",\"line\":{},\"column\":{},\"length\":{},\"text\":\"{}\"}},\"suggestions\":[{}]}}",
            self.code,
            self.severity,
            escape_json(&self.message),
            escape_json(&self.file),
            self.line,
            self.column,
            self.length,
            escape_json(&self.text),
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

        assert!(diagnostic.json().contains("bad \\\"token\\\""));
        assert!(diagnostic.json().contains("let x = \\\"a\\\""));
    }
}
