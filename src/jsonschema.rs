/// Minimal JSON Schema (Draft-7 subset) validator.
///
/// Supported keywords:
///   type, required, properties, additionalProperties,
///   minimum, maximum, minLength, maxLength, enum, items
///
/// This implementation avoids the heavy `jsonschema` crate (and its transitive
/// dependencies) while covering the features used in ClawEval suites.
use serde_json::Value;

pub fn validate(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    validate_inner(schema, instance, "$", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_inner(schema: &Value, instance: &Value, path: &str, errors: &mut Vec<String>) {
    let obj = match schema.as_object() {
        Some(o) => o,
        None => return, // non-object schema: always pass (permissive)
    };

    // "type"
    if let Some(type_val) = obj.get("type") {
        let expected_types: Vec<&str> = match type_val {
            Value::String(s) => vec![s.as_str()],
            Value::Array(arr) => arr.iter().filter_map(|v| v.as_str()).collect(),
            _ => vec![],
        };
        if !expected_types.is_empty() && !type_matches(instance, &expected_types) {
            errors.push(format!(
                "{}: expected type {:?}, got {}",
                path,
                expected_types,
                json_type_name(instance)
            ));
        }
    }

    // "enum"
    if let Some(Value::Array(variants)) = obj.get("enum") {
        if !variants.contains(instance) {
            errors.push(format!("{}: value not in enum {:?}", path, variants));
        }
    }

    // "minimum" / "maximum" (numeric)
    if let Some(n) = instance.as_f64() {
        if let Some(min) = obj.get("minimum").and_then(|v| v.as_f64()) {
            if n < min {
                errors.push(format!("{}: {} < minimum {}", path, n, min));
            }
        }
        if let Some(max) = obj.get("maximum").and_then(|v| v.as_f64()) {
            if n > max {
                errors.push(format!("{}: {} > maximum {}", path, n, max));
            }
        }
    }

    // "minLength" / "maxLength" (string)
    if let Some(s) = instance.as_str() {
        let len = s.chars().count();
        if let Some(min) = obj.get("minLength").and_then(|v| v.as_u64()) {
            if (len as u64) < min {
                errors.push(format!("{}: string length {} < minLength {}", path, len, min));
            }
        }
        if let Some(max) = obj.get("maxLength").and_then(|v| v.as_u64()) {
            if (len as u64) > max {
                errors.push(format!("{}: string length {} > maxLength {}", path, len, max));
            }
        }
    }

    // "required" + "properties" + "additionalProperties"
    if let Some(Value::Object(instance_obj)) = Some(instance) {
        if let Some(Value::Object(props)) = obj.get("properties") {
            // Check required fields.
            if let Some(Value::Array(required)) = obj.get("required") {
                for req in required {
                    if let Some(field) = req.as_str() {
                        if !instance_obj.contains_key(field) {
                            errors.push(format!("{}: missing required field '{}'", path, field));
                        }
                    }
                }
            }

            // Validate each property against its sub-schema.
            for (key, sub_schema) in props {
                if let Some(value) = instance_obj.get(key.as_str()) {
                    let child_path = format!("{}.{}", path, key);
                    validate_inner(sub_schema, value, &child_path, errors);
                }
            }

            // additionalProperties: false
            if let Some(Value::Bool(false)) = obj.get("additionalProperties") {
                for key in instance_obj.keys() {
                    if !props.contains_key(key.as_str()) {
                        errors.push(format!(
                            "{}: additional property '{}' not allowed",
                            path, key
                        ));
                    }
                }
            }
        } else {
            // No "properties" but may still have "required".
            if let Some(Value::Array(required)) = obj.get("required") {
                if let Value::Object(instance_obj) = instance {
                    for req in required {
                        if let Some(field) = req.as_str() {
                            if !instance_obj.contains_key(field) {
                                errors.push(format!(
                                    "{}: missing required field '{}'",
                                    path, field
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // "items" (array)
    if let (Some(items_schema), Value::Array(arr)) = (obj.get("items"), instance) {
        for (i, item) in arr.iter().enumerate() {
            let item_path = format!("{}[{}]", path, i);
            validate_inner(items_schema, item, &item_path, errors);
        }
    }
}

fn type_matches(instance: &Value, expected: &[&str]) -> bool {
    let actual = json_type_name(instance);
    for t in expected {
        if actual == *t {
            return true;
        }
        // JSON Schema: "integer" matches numbers with no fractional part.
        if *t == "integer" {
            if let Some(n) = instance.as_f64() {
                if n.fract() == 0.0 {
                    return true;
                }
            }
        }
    }
    false
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
