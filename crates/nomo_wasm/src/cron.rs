pub(crate) const MAX_EXPRESSION_BYTES: usize = 256;
pub(crate) const SEARCH_MINUTES: u64 = 4_208_400;
pub(crate) const MAX_TIMESTAMP_MILLIS: i64 = 253_402_300_799_999;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CronError {
    pub code: &'static str,
    pub message: &'static str,
    pub field: u64,
}

impl CronError {
    fn new(code: &'static str, field: usize) -> Self {
        let message = match code {
            "range" => "cron field value is outside its range",
            "limit" => "cron limit exceeded",
            "timestamp_range" => "cron timestamp is outside the supported range",
            "no_match" => "cron schedule has no later matching minute",
            _ => "invalid cron expression syntax",
        };
        Self {
            code,
            message,
            field: field as u64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSchedule {
    minute: u64,
    hour: u64,
    day_of_month: u64,
    month: u64,
    day_of_week: u64,
}

pub(crate) fn parse(expression: &str) -> Result<(), CronError> {
    parse_schedule(expression).map(|_| ())
}

pub(crate) fn matches(expression: &str, unix_millis: i64) -> Result<bool, CronError> {
    validate_timestamp(unix_millis)?;
    let parsed = parse_schedule(expression)?;
    Ok(minute_matches(&parsed, unix_millis / 60_000))
}

pub(crate) fn next_after(expression: &str, unix_millis: i64) -> Result<i64, CronError> {
    validate_timestamp(unix_millis)?;
    let parsed = parse_schedule(expression)?;
    let maximum_minute = MAX_TIMESTAMP_MILLIS / 60_000;
    for candidate in (unix_millis / 60_000 + 1..).take(SEARCH_MINUTES as usize) {
        if candidate > maximum_minute {
            break;
        }
        if minute_matches(&parsed, candidate) {
            return Ok(candidate * 60_000);
        }
    }
    Err(CronError::new("no_match", 5))
}

fn validate_timestamp(unix_millis: i64) -> Result<(), CronError> {
    if !(0..=MAX_TIMESTAMP_MILLIS).contains(&unix_millis) {
        return Err(CronError::new("timestamp_range", 5));
    }
    Ok(())
}

fn parse_schedule(expression: &str) -> Result<ParsedSchedule, CronError> {
    if expression.len() > MAX_EXPRESSION_BYTES {
        return Err(CronError::new("limit", 5));
    }
    if !expression.is_ascii() {
        return Err(CronError::new("syntax", 5));
    }
    let fields = expression.split_ascii_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(CronError::new("syntax", 5));
    }
    Ok(ParsedSchedule {
        minute: parse_field(fields[0], 0, 59, 0)?,
        hour: parse_field(fields[1], 0, 23, 1)?,
        day_of_month: parse_field(fields[2], 1, 31, 2)?,
        month: parse_field(fields[3], 1, 12, 3)?,
        day_of_week: parse_field(fields[4], 0, 6, 4)?,
    })
}

fn parse_field(text: &str, minimum: u64, maximum: u64, field: usize) -> Result<u64, CronError> {
    let mut mask = 0_u64;
    for member in text.split(',') {
        if member.is_empty() {
            return Err(CronError::new("syntax", field));
        }
        parse_member(member, minimum, maximum, field, &mut mask)?;
    }
    if mask == 0 {
        return Err(CronError::new("syntax", field));
    }
    Ok(mask)
}

fn parse_member(
    text: &str,
    minimum: u64,
    maximum: u64,
    field: usize,
    mask: &mut u64,
) -> Result<(), CronError> {
    let mut slash_parts = text.split('/');
    let base = slash_parts.next().unwrap_or_default();
    let step_text = slash_parts.next();
    if slash_parts.next().is_some() {
        return Err(CronError::new("syntax", field));
    }
    let step = match step_text {
        Some(value) => {
            let parsed = parse_unsigned(value, field)?;
            if parsed == 0 || parsed > maximum - minimum + 1 {
                return Err(CronError::new("range", field));
            }
            parsed
        }
        None => 1,
    };

    let (start, end) = if base == "*" {
        (minimum, maximum)
    } else if let Some((left, right)) = split_range(base, field)? {
        (parse_unsigned(left, field)?, parse_unsigned(right, field)?)
    } else {
        if step_text.is_some() {
            return Err(CronError::new("syntax", field));
        }
        let value = parse_unsigned(base, field)?;
        (value, value)
    };
    if start < minimum || start > maximum || end < minimum || end > maximum || start > end {
        return Err(CronError::new("range", field));
    }

    let mut value = start;
    loop {
        *mask |= 1_u64 << value;
        if end - value < step {
            break;
        }
        value += step;
    }
    Ok(())
}

fn split_range(text: &str, field: usize) -> Result<Option<(&str, &str)>, CronError> {
    let mut parts = text.split('-');
    let left = parts.next().unwrap_or_default();
    let Some(right) = parts.next() else {
        return Ok(None);
    };
    if parts.next().is_some() || left.is_empty() || right.is_empty() {
        return Err(CronError::new("syntax", field));
    }
    Ok(Some((left, right)))
}

fn parse_unsigned(text: &str, field: usize) -> Result<u64, CronError> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CronError::new("syntax", field));
    }
    text.parse::<u64>()
        .map_err(|_| CronError::new("range", field))
}

fn full_mask(minimum: u64, maximum: u64) -> u64 {
    let mut mask = 0_u64;
    for value in minimum..=maximum {
        mask |= 1_u64 << value;
    }
    mask
}

fn minute_matches(schedule: &ParsedSchedule, minute_index: i64) -> bool {
    let days = minute_index / 1_440;
    let minute = (minute_index % 60) as u64;
    let hour = ((minute_index / 60) % 24) as u64;
    let (_, month, day) = civil_from_days(days);
    let day_of_week = ((days + 4) % 7) as u64;

    if !contains(schedule.minute, minute)
        || !contains(schedule.hour, hour)
        || !contains(schedule.month, month)
    {
        return false;
    }
    let day_of_month_matches = contains(schedule.day_of_month, day);
    let day_of_week_matches = contains(schedule.day_of_week, day_of_week);
    let day_of_month_unrestricted = schedule.day_of_month == full_mask(1, 31);
    let day_of_week_unrestricted = schedule.day_of_week == full_mask(0, 6);
    match (day_of_month_unrestricted, day_of_week_unrestricted) {
        (true, true) => true,
        (true, false) => day_of_week_matches,
        (false, true) => day_of_month_matches,
        (false, false) => day_of_month_matches || day_of_week_matches,
    }
}

fn contains(mask: u64, value: u64) -> bool {
    mask & (1_u64 << value) != 0
}

fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let adjusted = days + 719_468;
    let era = adjusted / 146_097;
    let day_of_era = (adjusted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_field_forms_and_rejects_invalid_forms() {
        for expression in [
            "* * * * *",
            "0 12 1 1 0",
            "*/15 1-20/3 1,3,5 1-12 0-6",
            "0,15-30/5,59 0 * * *",
        ] {
            assert!(parse(expression).is_ok(), "{expression}");
        }
        for (expression, code, field) in [
            ("* * * *", "syntax", 5),
            ("* * * * * *", "syntax", 5),
            ("60 * * * *", "range", 0),
            ("* 24 * * *", "range", 1),
            ("* * 0 * *", "range", 2),
            ("* * * 13 *", "range", 3),
            ("* * * * 7", "range", 4),
            ("*/0 * * * *", "range", 0),
            ("1/2 * * * *", "syntax", 0),
            ("5-1 * * * *", "range", 0),
            ("1,,2 * * * *", "syntax", 0),
            ("MON * * * *", "syntax", 0),
        ] {
            let error = parse(expression).unwrap_err();
            assert_eq!((error.code, error.field), (code, field), "{expression}");
        }
        assert_eq!(
            parse(&"*".repeat(MAX_EXPRESSION_BYTES + 1))
                .unwrap_err()
                .code,
            "limit"
        );
        let exact = format!("{} 00 * * *", vec!["0"; 124].join(","));
        assert_eq!(exact.len(), MAX_EXPRESSION_BYTES);
        assert!(parse(&exact).is_ok());
        assert_eq!(parse(&format!("{exact} ")).unwrap_err().code, "limit");
    }

    #[test]
    fn calculates_epoch_leap_century_and_day_field_semantics() {
        assert!(matches("* * * * *", 0).unwrap());
        assert!(matches("0 0 1 1 *", 0).unwrap());
        assert!(!matches("0 0 2 1 *", 0).unwrap());
        assert_eq!(next_after("0 0 29 2 *", 0).unwrap(), 68_169_600_000);
        assert_eq!(
            next_after("0 0 29 2 *", 3_981_312_000_000).unwrap(),
            4_233_686_400_000
        );
        assert!(matches("0 0 2 * 4", 86_400_000).unwrap());
        assert!(matches("0 0 2 * 5", 86_400_000).unwrap());
        assert!(!matches("0 0 2 * 5", 172_800_000).unwrap());
        assert!(matches("0 0 */1 * 4", 0).unwrap());
    }

    #[test]
    fn next_is_strict_and_timestamp_range_is_bounded() {
        assert_eq!(next_after("* * * * *", 0).unwrap(), 60_000);
        assert_eq!(next_after("* * * * *", 59_999).unwrap(), 60_000);
        assert_eq!(next_after("* * * * *", 60_000).unwrap(), 120_000);
        assert_eq!(
            matches("* * * * *", -1).unwrap_err().code,
            "timestamp_range"
        );
        assert_eq!(
            next_after("* * * * *", MAX_TIMESTAMP_MILLIS)
                .unwrap_err()
                .code,
            "no_match"
        );
    }
}
