use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::io::BufRead;
use std::time::{Duration, Instant};

/// A single SSE event.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SseEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Result of an SSE subscription.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SseResult {
    pub events: Vec<SseEvent>,
    pub count: u32,
}

/// Subscribe to an SSE endpoint and collect events.
///
/// Reads the SSE stream line-by-line, parsing `event:`, `data:`, and `id:` fields.
/// Stops after `max_events` events (0 = no limit) or when `timeout` elapses.
/// Optionally filters by SSE `event:` type.
pub fn subscribe(
    url: &str,
    headers: Option<&HashMap<String, String>>,
    timeout: Duration,
    max_events: u32,
    event_filter: Option<&str>,
) -> Result<SseResult> {
    let mut req = ureq::get(url)
        .set("Accept", "text/event-stream")
        .set("Cache-Control", "no-cache");

    if let Some(hs) = headers {
        for (k, v) in hs {
            req = req.set(k, v);
        }
    }

    // Set read timeout for the underlying connection
    req = req.timeout(timeout);

    let response = req.call().map_err(|e| anyhow!("SSE connect failed: {}", e))?;
    let reader = response.into_reader();
    let buf_reader = std::io::BufReader::new(reader);

    let mut events = Vec::new();
    let mut current_event: Option<String> = None;
    let mut current_data = Vec::new();
    let mut current_id: Option<String> = None;
    let start = Instant::now();

    for line_result in buf_reader.lines() {
        if start.elapsed() >= timeout {
            break;
        }
        if max_events > 0 && events.len() >= max_events as usize {
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(_) => break, // timeout or connection closed
        };

        if line.is_empty() {
            // Empty line = event boundary
            if !current_data.is_empty() {
                let data = current_data.join("\n");
                let event_type = current_event.take();

                let matches_filter = match event_filter {
                    Some(filter) => event_type.as_deref() == Some(filter),
                    None => true,
                };

                if matches_filter {
                    events.push(SseEvent {
                        event: event_type,
                        data,
                        id: current_id.take(),
                    });
                } else {
                    current_id = None;
                }
                current_data.clear();
            }
            continue;
        }

        if let Some(val) = line.strip_prefix("data:") {
            current_data.push(val.trim_start().to_string());
        } else if let Some(val) = line.strip_prefix("event:") {
            current_event = Some(val.trim_start().to_string());
        } else if let Some(val) = line.strip_prefix("id:") {
            current_id = Some(val.trim_start().to_string());
        }
        // Ignore comments (lines starting with ':') and unknown fields
    }

    // Flush any pending event
    if !current_data.is_empty() {
        let data = current_data.join("\n");
        let event_type = current_event.take();
        let matches_filter = match event_filter {
            Some(filter) => event_type.as_deref() == Some(filter),
            None => true,
        };
        if matches_filter {
            events.push(SseEvent {
                event: event_type,
                data,
                id: current_id.take(),
            });
        }
    }

    Ok(SseResult {
        count: events.len() as u32,
        events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test SSE line parsing logic without a real server.
    #[test]
    fn parse_sse_lines() {
        let raw = "event: message\ndata: hello world\nid: 1\n\ndata: second\n\n";
        let lines: Vec<&str> = raw.lines().collect();

        let mut events = Vec::new();
        let mut current_event: Option<String> = None;
        let mut current_data = Vec::new();
        let mut current_id: Option<String> = None;

        for line in lines {
            if line.is_empty() {
                if !current_data.is_empty() {
                    events.push(SseEvent {
                        event: current_event.take(),
                        data: current_data.join("\n"),
                        id: current_id.take(),
                    });
                    current_data.clear();
                }
                continue;
            }
            if let Some(val) = line.strip_prefix("data: ") {
                current_data.push(val.to_string());
            } else if let Some(val) = line.strip_prefix("event: ") {
                current_event = Some(val.to_string());
            } else if let Some(val) = line.strip_prefix("id: ") {
                current_id = Some(val.to_string());
            }
        }

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.as_deref(), Some("message"));
        assert_eq!(events[0].data, "hello world");
        assert_eq!(events[0].id.as_deref(), Some("1"));
        assert_eq!(events[1].event, None);
        assert_eq!(events[1].data, "second");
    }
}
