use nomo::project::{ProjectTestReport, ProjectTestStatus};

pub(super) fn print_test_reports(reports: &[ProjectTestReport]) {
    let total = reports
        .iter()
        .map(|report| report.tests.len())
        .sum::<usize>();
    println!("running {total} tests");
    for report in reports {
        for test in &report.tests {
            match test.status {
                ProjectTestStatus::Ok => println!("ok {}", test.name),
                ProjectTestStatus::Failed => {
                    println!("fail {}", test.name);
                    if let Some(message) = test.message.as_deref() {
                        println!("  {message}");
                    }
                }
            }
        }
    }
}

pub(super) fn reports_have_failures(reports: &[ProjectTestReport]) -> bool {
    reports.iter().any(ProjectTestReport::has_failures)
}

pub(super) fn test_reports_json(reports: &[ProjectTestReport]) -> String {
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
                ProjectTestStatus::Ok => "ok",
                ProjectTestStatus::Failed => "failed",
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
