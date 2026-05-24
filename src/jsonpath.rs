use serde_json::Value;

enum Step {
    Key(String),
    Index(usize),
    Wildcard,
}

fn parse(path: &str) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let trimmed = path.trim().trim_start_matches('$');
    for raw in trimmed.split('.') {
        if raw.is_empty() {
            continue;
        }
        let mut rest = raw;
        // A segment may be "name", "name[0]", "name[*]", or "[0]".
        while let Some(lb) = rest.find('[') {
            let key = &rest[..lb];
            if !key.is_empty() {
                steps.push(Step::Key(key.to_string()));
            }
            let rb_rel = rest[lb..].find(']')?;
            let rb = lb + rb_rel;
            let inner = &rest[lb + 1..rb];
            if inner == "*" {
                steps.push(Step::Wildcard);
            } else {
                steps.push(Step::Index(inner.parse().ok()?));
            }
            rest = &rest[rb + 1..];
        }
        if !rest.is_empty() {
            steps.push(Step::Key(rest.to_string()));
        }
    }
    Some(steps)
}

/// Evaluate a minimal JSON path (`$.a.b`, `a[0].c`, `errors[*].code`) over a
/// JSON value, returning all matched values. Returns empty on a bad path.
pub fn eval(value: &Value, path: &str) -> Vec<Value> {
    let Some(steps) = parse(path) else {
        return Vec::new();
    };
    let mut current: Vec<&Value> = vec![value];
    for step in &steps {
        let mut next: Vec<&Value> = Vec::new();
        for v in &current {
            match step {
                Step::Key(k) => {
                    if let Some(child) = v.get(k) {
                        next.push(child);
                    }
                }
                Step::Index(i) => {
                    if let Some(child) = v.get(i) {
                        next.push(child);
                    }
                }
                Step::Wildcard => match v {
                    Value::Array(arr) => next.extend(arr.iter()),
                    Value::Object(map) => next.extend(map.values()),
                    _ => {}
                },
            }
        }
        current = next;
    }
    current.into_iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::eval;
    use serde_json::json;

    #[test]
    fn nested_key() {
        let v = json!({"a": {"b": 1}});
        assert_eq!(eval(&v, "$.a.b"), vec![json!(1)]);
    }

    #[test]
    fn array_index() {
        let v = json!({"errors": [{"code": "E1"}, {"code": "E2"}]});
        assert_eq!(eval(&v, "errors[0].code"), vec![json!("E1")]);
    }

    #[test]
    fn array_wildcard() {
        let v = json!({"errors": [{"code": "E1"}, {"code": "E2"}]});
        assert_eq!(eval(&v, "$.errors[*].code"), vec![json!("E1"), json!("E2")]);
    }

    #[test]
    fn missing_path_is_empty() {
        let v = json!({"a": 1});
        assert!(eval(&v, "$.nope.x").is_empty());
    }

    #[test]
    fn invalid_index_is_empty() {
        let v = json!({"a": [1]});
        assert!(eval(&v, "a[x]").is_empty());
    }
}
