#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReport {
    pub project: String,
    pub tests: Vec<TestCaseResult>,
}

impl TestReport {
    pub fn has_failures(&self) -> bool {
        self.tests
            .iter()
            .any(|test| test.status == TestStatus::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCaseResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u128,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Ok,
    Failed,
}

pub fn reports_have_failures(reports: &[TestReport]) -> bool {
    reports.iter().any(TestReport::has_failures)
}

pub fn text_report(reports: &[TestReport]) -> String {
    let mut output = String::new();
    let total = reports
        .iter()
        .map(|report| report.tests.len())
        .sum::<usize>();
    output.push_str(&format!("running {total} tests\n"));
    for report in reports {
        for test in &report.tests {
            match test.status {
                TestStatus::Ok => output.push_str(&format!("ok {}\n", test.name)),
                TestStatus::Failed => {
                    output.push_str(&format!("fail {}\n", test.name));
                    if let Some(message) = test.message.as_deref() {
                        output.push_str(&format!("  {message}\n"));
                    }
                }
            }
        }
    }
    output
}

pub fn json_report(reports: &[TestReport]) -> String {
    let status = if reports_have_failures(reports) {
        "failed"
    } else {
        "ok"
    };
    let mut json = format!("{{\"status\":\"{status}\",\"tests\":[");
    let mut first = true;
    for report in reports {
        for test in &report.tests {
            if !first {
                json.push(',');
            }
            first = false;
            let status = match test.status {
                TestStatus::Ok => "ok",
                TestStatus::Failed => "failed",
            };
            json.push_str(&format!(
                "{{\"name\":\"{}\",\"status\":\"{}\",\"duration_ms\":{}",
                json_escape(&test.name),
                status,
                test.duration_ms
            ));
            if let Some(message) = test.message.as_deref() {
                json.push_str(&format!(",\"message\":\"{}\"", json_escape(message)));
            }
            json.push('}');
        }
    }
    json.push_str("]}");
    json
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            ch if ch.is_control() => format!("\\u{:04x}", ch as u32).chars().collect(),
            ch => vec![ch],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_failures() {
        let report = TestReport {
            project: "local/app".to_string(),
            tests: vec![TestCaseResult {
                name: "app.main.fails".to_string(),
                status: TestStatus::Failed,
                duration_ms: 7,
                message: Some("boom".to_string()),
            }],
        };

        assert!(report.has_failures());
        assert!(reports_have_failures(&[report]));
    }

    #[test]
    fn renders_text_report() {
        let report = TestReport {
            project: "local/app".to_string(),
            tests: vec![
                TestCaseResult {
                    name: "app.main.passes".to_string(),
                    status: TestStatus::Ok,
                    duration_ms: 1,
                    message: None,
                },
                TestCaseResult {
                    name: "app.main.fails".to_string(),
                    status: TestStatus::Failed,
                    duration_ms: 2,
                    message: Some("boom".to_string()),
                },
            ],
        };

        assert_eq!(
            text_report(&[report]),
            "running 2 tests\nok app.main.passes\nfail app.main.fails\n  boom\n"
        );
    }

    #[test]
    fn renders_json_report() {
        let report = TestReport {
            project: "local/app".to_string(),
            tests: vec![TestCaseResult {
                name: "app.main.quote".to_string(),
                status: TestStatus::Failed,
                duration_ms: 3,
                message: Some("quote: \"x\"\nnext".to_string()),
            }],
        };

        assert_eq!(
            json_report(&[report]),
            "{\"status\":\"failed\",\"tests\":[{\"name\":\"app.main.quote\",\"status\":\"failed\",\"duration_ms\":3,\"message\":\"quote: \\\"x\\\"\\nnext\"}]}"
        );
    }
}
