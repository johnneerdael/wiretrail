use serde_json::Value;

#[derive(Debug, Default)]
pub struct ErrorFields {
    pub message: Option<String>,
    pub code: Option<String>,
}

const MESSAGE_KEYS: &[&str] = &["message", "error_description", "error", "reason", "detail", "details"];
const CODE_KEYS: &[&str] = &["code", "error_code", "status"];

/// Extract common error fields from a JSON response body. Returns empty fields
/// for non-JSON or when keys are absent.
pub fn parse_error_fields(body: &str) -> ErrorFields {
    let mut fields = ErrorFields::default();
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return fields;
    };
    fields.message = first_string(&v, MESSAGE_KEYS);
    fields.code = first_string(&v, CODE_KEYS);
    fields
}

fn first_string(v: &Value, keys: &[&str]) -> Option<String> {
    let obj = v.as_object()?;
    for k in keys {
        match obj.get(*k) {
            Some(Value::String(s)) => return Some(s.clone()),
            Some(Value::Number(n)) => return Some(n.to_string()),
            _ => {}
        }
    }
    // Fall back to a nested "error" object.
    if let Some(Value::Object(err)) = obj.get("error") {
        for k in keys {
            match err.get(*k) {
                Some(Value::String(s)) => return Some(s.clone()),
                Some(Value::Number(n)) => return Some(n.to_string()),
                _ => {}
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_error_fields;

    #[test]
    fn extracts_message_and_code_flat() {
        let f = parse_error_fields(r#"{"message":"Not found","code":"NF404"}"#);
        assert_eq!(f.message.as_deref(), Some("Not found"));
        assert_eq!(f.code.as_deref(), Some("NF404"));
    }

    #[test]
    fn extracts_from_nested_error_object() {
        let f = parse_error_fields(r#"{"error":{"message":"bad token","code":"E1"}}"#);
        assert_eq!(f.message.as_deref(), Some("bad token"));
        assert_eq!(f.code.as_deref(), Some("E1"));
    }

    #[test]
    fn numeric_code_becomes_string() {
        let f = parse_error_fields(r#"{"error":"boom","status":500}"#);
        assert_eq!(f.message.as_deref(), Some("boom"));
        assert_eq!(f.code.as_deref(), Some("500"));
    }

    #[test]
    fn non_json_is_empty() {
        let f = parse_error_fields("<html>500</html>");
        assert!(f.message.is_none());
        assert!(f.code.is_none());
    }
}
