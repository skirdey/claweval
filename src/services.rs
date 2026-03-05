use crate::spec::ServiceSpec;
use anyhow::{anyhow, Result};
use std::io::BufRead;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Manages background service processes for a suite.
pub struct ServiceManager {
    children: Vec<(String, Child)>,
}

impl ServiceManager {
    /// Spawn all declared services. Waits for each to become ready (if `ready_text` is set).
    pub fn start(specs: &[ServiceSpec]) -> Result<Self> {
        let mut children = Vec::new();

        for spec in specs {
            let mut cmd = Command::new(&spec.command);
            if let Some(args) = &spec.args {
                cmd.args(args);
            }
            if let Some(env) = &spec.env {
                for (k, v) in env {
                    cmd.env(k, v);
                }
            }

            // Capture stdout for ready detection
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            let mut child = cmd.spawn().map_err(|e| {
                anyhow!("failed to start service '{}' ({}): {}", spec.name, spec.command, e)
            })?;

            // Wait for ready text if specified
            if let Some(ready_text) = &spec.ready_text {
                let timeout = Duration::from_millis(spec.ready_timeout_ms.unwrap_or(10_000));
                wait_for_ready(&spec.name, &mut child, ready_text, timeout)?;
            }

            children.push((spec.name.clone(), child));
        }

        Ok(Self { children })
    }

    /// Stop all services (SIGTERM on Unix, TerminateProcess on Windows).
    pub fn stop(&mut self) {
        for (name, child) in &mut self.children {
            if let Err(e) = child.kill() {
                eprintln!("[services] failed to kill '{}': {}", name, e);
            }
            if let Err(e) = child.wait() {
                eprintln!("[services] failed to wait on '{}': {}", name, e);
            }
        }
        self.children.clear();
    }
}

impl Drop for ServiceManager {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_for_ready(name: &str, child: &mut Child, ready_text: &str, timeout: Duration) -> Result<()> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("no stdout for service '{}'", name))?;

    let start = Instant::now();
    let reader = std::io::BufReader::new(stdout);

    for line_result in reader.lines() {
        if start.elapsed() >= timeout {
            return Err(anyhow!(
                "service '{}' did not become ready within {}ms (waiting for '{}')",
                name,
                timeout.as_millis(),
                ready_text
            ));
        }
        match line_result {
            Ok(line) => {
                if line.contains(ready_text) {
                    return Ok(());
                }
            }
            Err(e) => {
                return Err(anyhow!(
                    "error reading stdout from service '{}': {}",
                    name,
                    e
                ));
            }
        }
    }

    Err(anyhow!(
        "service '{}' stdout closed before ready text '{}' was found",
        name,
        ready_text
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_stop_echo_service() {
        let specs = vec![ServiceSpec {
            name: "echo-test".to_string(),
            command: "echo".to_string(),
            args: Some(vec!["service ready".to_string()]),
            env: None,
            ready_text: Some("ready".to_string()),
            ready_timeout_ms: Some(5000),
        }];
        let mut mgr = ServiceManager::start(&specs).expect("should start echo service");
        mgr.stop();
    }

    #[test]
    fn service_without_ready_text() {
        let specs = vec![ServiceSpec {
            name: "sleep-test".to_string(),
            command: "sleep".to_string(),
            args: Some(vec!["0.1".to_string()]),
            env: None,
            ready_text: None,
            ready_timeout_ms: None,
        }];
        let mut mgr = ServiceManager::start(&specs).expect("should start sleep service");
        mgr.stop();
    }
}
