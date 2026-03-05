use anyhow::{anyhow, Context, Result};
use serde_json::Value;

pub struct UreqResponse {
    pub status: u16,
    pub body: String,
}

pub enum UreqRead {
    Ok(UreqResponse),
    ErrorStatus(UreqResponse),
}

pub fn parse_embedded_json(text: &str) -> Option<Value> {
    fn parse_or_none(s: &str) -> Option<Value> {
        serde_json::from_str::<Value>(s).ok()
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(v) = parse_or_none(trimmed) {
        return Some(v);
    }

    let first_obj = trimmed.find('{');
    let last_obj = trimmed.rfind('}');
    if let (Some(a), Some(b)) = (first_obj, last_obj) {
        if b > a {
            let body = &trimmed[a..=b];
            if let Some(v) = parse_or_none(body) {
                return Some(v);
            }
        }
    }

    let first_arr = trimmed.find('[');
    let last_arr = trimmed.rfind(']');
    if let (Some(a), Some(b)) = (first_arr, last_arr) {
        if b > a {
            let body = &trimmed[a..=b];
            if let Some(v) = parse_or_none(body) {
                return Some(v);
            }
        }
    }

    None
}

pub fn read_ureq_response(context: &str, url: &str, result: Result<ureq::Response, ureq::Error>) -> Result<UreqRead> {
    match result {
        Ok(resp) => {
            let status = resp.status();
            let body = resp
                .into_string()
                .with_context(|| format!("failed to read response body from {}", url))?;
            Ok(UreqRead::Ok(UreqResponse { status, body }))
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Ok(UreqRead::ErrorStatus(UreqResponse { status: code, body }))
        }
        Err(e) => Err(anyhow!("{} request to {} failed: {}", context, url, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_embedded_json_plain() {
        let v = parse_embedded_json("{\"ok\":true}")
            .expect("expected plain JSON to parse");
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn parse_embedded_json_embedded_object() {
        let v = parse_embedded_json("noise {\"ok\":true} noise")
            .expect("expected embedded object JSON to parse");
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn parse_embedded_json_embedded_array() {
        let v = parse_embedded_json("noise [1,2,3] noise")
            .expect("expected embedded array JSON to parse");
        assert_eq!(v.as_array().expect("array").len(), 3);
    }

    #[test]
    fn parse_embedded_json_empty_and_malformed() {
        assert!(parse_embedded_json("").is_none());
        assert!(parse_embedded_json("not-json").is_none());
    }
}
