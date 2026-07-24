use crate::json::{self, JsonNode, JsonNodeValue};

pub(crate) const MAX_MESSAGE_BYTES: usize = 1_048_575;
const MAX_CHUNK_BYTES: usize = 1_048_576;
const MAX_COMBINED_BYTES: usize = 2_097_151;
const MAX_BATCH_MESSAGES: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageKind {
    Request,
    Notification,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtocolError {
    pub code: &'static str,
    pub message: &'static str,
}

impl ProtocolError {
    fn new(code: &'static str) -> Self {
        let message = match code {
            "limit" => "JSON-RPC limit exceeded",
            "framing" => "invalid JSON-RPC newline framing",
            "json" => "invalid bounded JSON input",
            "protocol" => "invalid JSON-RPC 2.0 envelope",
            _ => "invalid JSON-RPC argument",
        };
        Self { code, message }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decoder {
    pub pending: String,
    pub max_message_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecodeBatch {
    pub decoder: Decoder,
    pub messages: Vec<String>,
}

pub(crate) fn decoder(max_message_bytes: u64) -> Result<Decoder, ProtocolError> {
    let max_message_bytes =
        usize::try_from(max_message_bytes).map_err(|_| ProtocolError::new("invalid_request"))?;
    if !valid_limit(max_message_bytes) {
        return Err(ProtocolError::new("invalid_request"));
    }
    Ok(Decoder {
        pending: String::new(),
        max_message_bytes,
    })
}

pub(crate) fn feed(decoder: Decoder, chunk: &str) -> Result<DecodeBatch, ProtocolError> {
    if !valid_limit(decoder.max_message_bytes)
        || decoder.pending.len() > decoder.max_message_bytes
        || decoder.pending.contains('\n')
    {
        return Err(ProtocolError::new("invalid_request"));
    }
    if chunk.len() > MAX_CHUNK_BYTES {
        return Err(ProtocolError::new("limit"));
    }
    let combined_len = decoder
        .pending
        .len()
        .checked_add(chunk.len())
        .filter(|len| *len <= MAX_COMBINED_BYTES)
        .ok_or_else(|| ProtocolError::new("limit"))?;
    let mut combined = String::with_capacity(combined_len);
    combined.push_str(&decoder.pending);
    combined.push_str(chunk);

    let mut messages = Vec::new();
    let mut line_start = 0;
    for (index, byte) in combined.bytes().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let mut line_end = index;
        if line_end > line_start && combined.as_bytes()[line_end - 1] == b'\r' {
            line_end -= 1;
        }
        if line_end == line_start {
            return Err(ProtocolError::new("framing"));
        }
        if messages.len() >= MAX_BATCH_MESSAGES {
            return Err(ProtocolError::new("limit"));
        }
        let raw = &combined[line_start..line_end];
        validate(raw, decoder.max_message_bytes)?;
        messages.push(raw.to_string());
        line_start = index + 1;
    }
    let pending = combined[line_start..].to_string();
    if pending.len() > decoder.max_message_bytes {
        return Err(ProtocolError::new("limit"));
    }
    Ok(DecodeBatch {
        decoder: Decoder {
            pending,
            max_message_bytes: decoder.max_message_bytes,
        },
        messages,
    })
}

pub(crate) fn finish(decoder: &Decoder) -> Result<(), ProtocolError> {
    if !valid_limit(decoder.max_message_bytes)
        || decoder.pending.len() > decoder.max_message_bytes
        || decoder.pending.contains('\n')
    {
        return Err(ProtocolError::new("invalid_request"));
    }
    if decoder.pending.is_empty() {
        Ok(())
    } else {
        Err(ProtocolError::new("framing"))
    }
}

pub(crate) fn parse(raw: &str, limit: u64) -> Result<String, ProtocolError> {
    let limit = checked_limit(limit)?;
    validate(raw, limit)?;
    Ok(raw.to_string())
}

pub(crate) fn encode(raw: &str, limit: u64) -> Result<String, ProtocolError> {
    let limit = checked_limit(limit)?;
    validate(raw, limit)?;
    let mut encoded = String::with_capacity(raw.len() + 1);
    encoded.push_str(raw);
    encoded.push('\n');
    Ok(encoded)
}

pub(crate) fn kind(raw: &str) -> Result<MessageKind, ProtocolError> {
    validate(raw, MAX_MESSAGE_BYTES)
}

pub(crate) fn request(
    id: &str,
    method: &str,
    params: Option<&str>,
) -> Result<String, ProtocolError> {
    let id_node = json::parse(id).map_err(map_json_error)?;
    if !request_id(&id_node.value) {
        return Err(ProtocolError::new("protocol"));
    }
    if let Some(params) = params {
        let params_node = json::parse(params).map_err(map_json_error)?;
        if !structured(&params_node.value) {
            return Err(ProtocolError::new("protocol"));
        }
    }
    let mut members = vec![
        ("jsonrpc".to_string(), json_string("2.0")?),
        ("id".to_string(), trimmed(id).to_string()),
        ("method".to_string(), json_string(method)?),
    ];
    if let Some(params) = params {
        members.push(("params".to_string(), trimmed(params).to_string()));
    }
    construct(members)
}

pub(crate) fn notification(method: &str, params: Option<&str>) -> Result<String, ProtocolError> {
    if let Some(params) = params {
        let params_node = json::parse(params).map_err(map_json_error)?;
        if !structured(&params_node.value) {
            return Err(ProtocolError::new("protocol"));
        }
    }
    let mut members = vec![
        ("jsonrpc".to_string(), json_string("2.0")?),
        ("method".to_string(), json_string(method)?),
    ];
    if let Some(params) = params {
        members.push(("params".to_string(), trimmed(params).to_string()));
    }
    construct(members)
}

pub(crate) fn success(id: &str, result: &str) -> Result<String, ProtocolError> {
    let id_node = json::parse(id).map_err(map_json_error)?;
    json::parse(result).map_err(map_json_error)?;
    if !response_id(&id_node.value) {
        return Err(ProtocolError::new("protocol"));
    }
    construct(vec![
        ("jsonrpc".to_string(), json_string("2.0")?),
        ("id".to_string(), trimmed(id).to_string()),
        ("result".to_string(), trimmed(result).to_string()),
    ])
}

pub(crate) fn failure(
    id: &str,
    code: i64,
    message: &str,
    data: Option<&str>,
) -> Result<String, ProtocolError> {
    let id_node = json::parse(id).map_err(map_json_error)?;
    if !response_id(&id_node.value) {
        return Err(ProtocolError::new("protocol"));
    }
    if let Some(data) = data {
        json::parse(data).map_err(map_json_error)?;
    }
    let mut error_members = vec![
        ("code".to_string(), code.to_string()),
        ("message".to_string(), json_string(message)?),
    ];
    if let Some(data) = data {
        error_members.push(("data".to_string(), trimmed(data).to_string()));
    }
    let error = json::from_object(&error_members).map_err(map_json_error)?;
    construct(vec![
        ("jsonrpc".to_string(), json_string("2.0")?),
        ("id".to_string(), trimmed(id).to_string()),
        ("error".to_string(), error),
    ])
}

fn construct(members: Vec<(String, String)>) -> Result<String, ProtocolError> {
    let raw = json::from_object(&members).map_err(map_json_error)?;
    validate(&raw, MAX_MESSAGE_BYTES)?;
    Ok(raw)
}

fn json_string(value: &str) -> Result<String, ProtocolError> {
    json::from_string(value).map_err(map_json_error)
}

fn checked_limit(limit: u64) -> Result<usize, ProtocolError> {
    let limit = usize::try_from(limit).map_err(|_| ProtocolError::new("invalid_request"))?;
    if valid_limit(limit) {
        Ok(limit)
    } else {
        Err(ProtocolError::new("invalid_request"))
    }
}

fn valid_limit(limit: usize) -> bool {
    (1..=MAX_MESSAGE_BYTES).contains(&limit)
}

fn validate(raw: &str, limit: usize) -> Result<MessageKind, ProtocolError> {
    if !valid_limit(limit) {
        return Err(ProtocolError::new("invalid_request"));
    }
    if raw.len() > limit {
        return Err(ProtocolError::new("limit"));
    }
    if raw.contains('\n') || raw.contains('\r') {
        return Err(ProtocolError::new("framing"));
    }
    let node = json::parse(raw).map_err(map_json_error)?;
    validate_envelope(&node)
}

fn validate_envelope(node: &JsonNode) -> Result<MessageKind, ProtocolError> {
    let JsonNodeValue::Object(members) = &node.value else {
        return Err(ProtocolError::new("protocol"));
    };
    let mut version = None;
    let mut method = None;
    let mut id = None;
    let mut params = None;
    let mut result = None;
    let mut error = None;
    for (key, value) in members {
        let slot = match key.as_str() {
            "jsonrpc" => &mut version,
            "method" => &mut method,
            "id" => &mut id,
            "params" => &mut params,
            "result" => &mut result,
            "error" => &mut error,
            _ => continue,
        };
        if slot.replace(value).is_some() {
            return Err(ProtocolError::new("protocol"));
        }
    }
    if !matches!(
        version.map(|value| &value.value),
        Some(JsonNodeValue::String(value)) if value == "2.0"
    ) {
        return Err(ProtocolError::new("protocol"));
    }
    if let Some(method) = method {
        if !matches!(method.value, JsonNodeValue::String(_))
            || result.is_some()
            || error.is_some()
            || params.is_some_and(|value| !structured(&value.value))
        {
            return Err(ProtocolError::new("protocol"));
        }
        return match id {
            None => Ok(MessageKind::Notification),
            Some(id) if request_id(&id.value) => Ok(MessageKind::Request),
            _ => Err(ProtocolError::new("protocol")),
        };
    }
    if params.is_some() || id.is_none() || result.is_some() == error.is_some() {
        return Err(ProtocolError::new("protocol"));
    }
    if !response_id(&id.expect("checked present").value) {
        return Err(ProtocolError::new("protocol"));
    }
    if let Some(error) = error {
        validate_error(&error.value)?;
        Ok(MessageKind::Error)
    } else {
        Ok(MessageKind::Success)
    }
}

fn validate_error(value: &JsonNodeValue) -> Result<(), ProtocolError> {
    let JsonNodeValue::Object(members) = value else {
        return Err(ProtocolError::new("protocol"));
    };
    let mut code = None;
    let mut message = None;
    let mut data = None;
    for (key, value) in members {
        let slot = match key.as_str() {
            "code" => &mut code,
            "message" => &mut message,
            "data" => &mut data,
            _ => continue,
        };
        if slot.replace(value).is_some() {
            return Err(ProtocolError::new("protocol"));
        }
    }
    if !matches!(
        code.map(|value| &value.value),
        Some(JsonNodeValue::Number(value)) if value.parse::<i64>().is_ok()
    ) || !matches!(
        message.map(|value| &value.value),
        Some(JsonNodeValue::String(_))
    ) {
        return Err(ProtocolError::new("protocol"));
    }
    Ok(())
}

fn request_id(value: &JsonNodeValue) -> bool {
    matches!(value, JsonNodeValue::String(_) | JsonNodeValue::Number(_))
}

fn response_id(value: &JsonNodeValue) -> bool {
    request_id(value) || matches!(value, JsonNodeValue::Null)
}

fn structured(value: &JsonNodeValue) -> bool {
    matches!(value, JsonNodeValue::Object(_) | JsonNodeValue::Array(_))
}

fn trimmed(raw: &str) -> &str {
    raw.trim_matches(|ch| matches!(ch, ' ' | '\n' | '\r' | '\t'))
}

fn map_json_error(error: json::JsonError) -> ProtocolError {
    ProtocolError::new(if error.code == "limit" {
        "limit"
    } else {
        "json"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_fragmented_and_coalesced_lines() {
        let decoder = decoder(4_096).unwrap();
        let first = feed(decoder, r#"{"jsonrpc":"2.0","id":1,"meth"#).unwrap();
        assert!(first.messages.is_empty());
        let second = feed(
            first.decoder,
            "od\":\"initialize\"}\r\n{\"jsonrpc\":\"2.0\",\"method\":\"ready\"}\n",
        )
        .unwrap();
        assert_eq!(second.messages.len(), 2);
        assert_eq!(kind(&second.messages[0]), Ok(MessageKind::Request));
        assert_eq!(kind(&second.messages[1]), Ok(MessageKind::Notification));
        assert_eq!(finish(&second.decoder), Ok(()));
    }

    #[test]
    fn rejects_duplicate_reserved_fields_without_echoing_input() {
        let secret = r#"{"jsonrpc":"2.0","method":"NOMO_JSONRPC_SECRET","method":"duplicate"}"#;
        let error = parse(secret, 4_096).unwrap_err();
        assert_eq!(error.code, "protocol");
        assert!(!error.message.contains("NOMO_JSONRPC_SECRET"));
    }

    #[test]
    fn enforces_exact_message_chunk_and_batch_boundaries() {
        assert_eq!(
            decoder(0).unwrap_err().code,
            "invalid_request",
            "zero limit must be rejected"
        );
        assert_eq!(
            decoder((MAX_MESSAGE_BYTES + 1) as u64).unwrap_err().code,
            "invalid_request",
            "limit above the transport maximum must be rejected"
        );

        let prefix = r#"{"jsonrpc":"2.0","method":"m","_pad":""#;
        let suffix = r#""}"#;
        let exact = format!(
            "{prefix}{}{suffix}",
            "x".repeat(MAX_MESSAGE_BYTES - prefix.len() - suffix.len())
        );
        assert_eq!(exact.len(), MAX_MESSAGE_BYTES);
        let exact_batch =
            feed(decoder(MAX_MESSAGE_BYTES as u64).unwrap(), &(exact + "\n")).unwrap();
        assert_eq!(exact_batch.messages.len(), 1);

        let oversized_chunk = "x".repeat(MAX_CHUNK_BYTES + 1);
        assert_eq!(
            feed(decoder(MAX_MESSAGE_BYTES as u64).unwrap(), &oversized_chunk)
                .unwrap_err()
                .code,
            "limit"
        );

        let line = "{\"jsonrpc\":\"2.0\",\"method\":\"m\"}\n";
        let exact_batch_lines = line.repeat(MAX_BATCH_MESSAGES);
        assert_eq!(
            feed(
                decoder(MAX_MESSAGE_BYTES as u64).unwrap(),
                &exact_batch_lines
            )
            .unwrap()
            .messages
            .len(),
            MAX_BATCH_MESSAGES
        );
        let too_many_lines = line.repeat(MAX_BATCH_MESSAGES + 1);
        assert_eq!(
            feed(decoder(MAX_MESSAGE_BYTES as u64).unwrap(), &too_many_lines)
                .unwrap_err()
                .code,
            "limit"
        );
    }
}
