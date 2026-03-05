use crate::runner::StepOutcome;
use std::collections::HashMap;

/// Replace all `{{var_name}}` placeholders in `template` with values from `vars`.
/// Unknown variables are left as-is.
pub fn interpolate(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, val) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, val);
    }
    result
}

/// Recursively interpolate all string values in a JSON tree.
pub fn interpolate_json(value: &mut serde_json::Value, vars: &HashMap<String, String>) {
    match value {
        serde_json::Value::String(s) => {
            *s = interpolate(s, vars);
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                interpolate_json(item, vars);
            }
        }
        serde_json::Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                interpolate_json(v, vars);
            }
        }
        _ => {}
    }
}

/// Extract a value from a step outcome's JSON using a JSON pointer.
/// Returns the value as a string (quotes stripped for string values).
pub fn extract_var(steps: &[StepOutcome], step_idx: usize, pointer: &str) -> Option<String> {
    let step = steps.get(step_idx)?;
    let json = step.response.as_ref()?.json.as_ref()?;
    let val = json.pointer(pointer)?;
    match val {
        serde_json::Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_replaces_known_vars() {
        let mut vars = HashMap::new();
        vars.insert("id".to_string(), "42".to_string());
        vars.insert("name".to_string(), "test".to_string());
        let result = interpolate("GET /api/{{id}}/{{name}}", &vars);
        assert_eq!(result, "GET /api/42/test");
    }

    #[test]
    fn interpolate_leaves_unknown_vars() {
        let vars = HashMap::new();
        let result = interpolate("{{unknown}} stays", &vars);
        assert_eq!(result, "{{unknown}} stays");
    }

    #[test]
    fn interpolate_json_recursive() {
        let mut vars = HashMap::new();
        vars.insert("url".to_string(), "http://localhost".to_string());
        let mut val = serde_json::json!({
            "endpoint": "{{url}}/api",
            "nested": { "path": "{{url}}/path" },
            "list": ["{{url}}/a", "{{url}}/b"],
            "number": 42
        });
        interpolate_json(&mut val, &vars);
        assert_eq!(val["endpoint"], "http://localhost/api");
        assert_eq!(val["nested"]["path"], "http://localhost/path");
        assert_eq!(val["list"][0], "http://localhost/a");
        assert_eq!(val["number"], 42);
    }

    #[test]
    fn extract_var_from_step() {
        use crate::backend::SendResponse;
        use crate::types::StepKind;
        use std::time::Duration;

        let steps = vec![StepOutcome {
            index: 0,
            kind: StepKind::HttpProbe,
            name: None,
            input: None,
            response: Some(SendResponse {
                output_text: String::new(),
                raw_stdout: String::new(),
                raw_stderr: String::new(),
                json: Some(serde_json::json!({"id": "abc-123", "count": 5})),
                duration: Duration::from_millis(1),
                exit_code: None,
            }),
            duration: Duration::from_millis(1),
            status_code: None,
            exit_code: None,
            poll_attempts: None,
            poll_satisfied: None,
            started_at: std::time::Instant::now(),
        }];
        assert_eq!(extract_var(&steps, 0, "/id"), Some("abc-123".to_string()));
        assert_eq!(extract_var(&steps, 0, "/count"), Some("5".to_string()));
        assert_eq!(extract_var(&steps, 0, "/missing"), None);
        assert_eq!(extract_var(&steps, 1, "/id"), None);
    }
}
