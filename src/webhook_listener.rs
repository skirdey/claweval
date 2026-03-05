use anyhow::{anyhow, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// Captured HTTP request from the webhook listener.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CapturedRequest {
    pub method: String,
    pub path: String,
    pub body: serde_json::Value,
}

/// Result of a webhook listen session.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WebhookResult {
    pub requests: Vec<CapturedRequest>,
    pub count: u32,
    pub port: u16,
}

/// Listen for incoming HTTP POST requests on `port` (0 = auto-pick).
/// Captures up to `min_requests` payloads or until `timeout` elapses.
/// Responds 200 OK to every request.
pub fn listen(
    port: u16,
    path_filter: &str,
    timeout: Duration,
    min_requests: u32,
) -> Result<WebhookResult> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .map_err(|e| anyhow!("webhook listener bind failed: {}", e))?;
    let actual_port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;

    let mut requests = Vec::new();
    let start = Instant::now();

    while start.elapsed() < timeout {
        if min_requests > 0 && requests.len() >= min_requests as usize {
            break;
        }

        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false)?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                if let Some(req) = parse_request(stream, path_filter) {
                    requests.push(req);
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(anyhow!("webhook accept error: {}", e)),
        }
    }

    Ok(WebhookResult {
        count: requests.len() as u32,
        requests,
        port: actual_port,
    })
}

fn parse_request(
    mut stream: std::net::TcpStream,
    path_filter: &str,
) -> Option<CapturedRequest> {
    let mut reader = BufReader::new(&stream);

    // Read request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line).ok()?;
    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        send_response(&mut stream, 400);
        return None;
    }
    let method = parts[0].to_string();
    let path = parts[1].to_string();

    // Read headers to find Content-Length
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("Content-Length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
        if let Some(val) = trimmed.strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }

    // Read body
    let body_json = if content_length > 0 {
        let mut body_buf = vec![0u8; content_length];
        std::io::Read::read_exact(&mut reader, &mut body_buf).ok()?;
        let body_str = String::from_utf8_lossy(&body_buf);
        serde_json::from_str(&body_str).unwrap_or(serde_json::Value::String(body_str.to_string()))
    } else {
        serde_json::Value::Null
    };

    send_response(&mut stream, 200);

    // Filter by path
    if !path_filter.is_empty() && path_filter != "/" && path != path_filter {
        return None;
    }

    Some(CapturedRequest {
        method,
        path,
        body: body_json,
    })
}

fn send_response(stream: &mut std::net::TcpStream, status: u16) {
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        status, reason
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;

    #[test]
    fn captures_post_request() {
        let result_handle = std::thread::spawn(|| {
            listen(0, "/", Duration::from_secs(3), 1)
        });

        // Give the listener time to bind
        std::thread::sleep(Duration::from_millis(100));

        // We need the port - use a different approach: start listener first
        // Actually we can't easily get the port from the thread. Let's use a channel.
        drop(result_handle);

        // Better test: use known port
        let port = 19876u16;
        let listener_handle = std::thread::spawn(move || {
            listen(port, "/callback", Duration::from_secs(3), 1)
        });

        std::thread::sleep(Duration::from_millis(200));

        // Send a POST request
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
            let body = r#"{"status":"done"}"#;
            let req = format!(
                "POST /callback HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(req.as_bytes());
            let _ = stream.flush();
        }

        let result = listener_handle.join().unwrap().expect("listen should succeed");
        assert_eq!(result.count, 1);
        assert_eq!(result.port, port);
        assert_eq!(result.requests[0].path, "/callback");
        assert_eq!(result.requests[0].body["status"], "done");
    }

    #[test]
    fn timeout_with_no_requests() {
        let result = listen(0, "/", Duration::from_millis(100), 0)
            .expect("listen should succeed with no requests");
        assert_eq!(result.count, 0);
        assert!(result.requests.is_empty());
    }
}
