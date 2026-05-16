use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use super::protocol::ToolCall;
use super::registry::ToolRegistry;

/// Execute a single tool call and return stdout/stderr
pub async fn execute_tool(
    registry: &ToolRegistry,
    call: &ToolCall,
    env_vars: &HashMap<String, String>,
) -> Result<String, String> {
    let tool = registry.get(&call.name)
        .ok_or_else(|| format!("Unknown tool: {}", call.name))?;

    execute_script(&tool.script_path, &call.args, env_vars).await
}

/// Execute a shell script with arguments, returning combined stdout+stderr
pub async fn execute_script(
    script_path: &Path,
    args: &str,
    env_vars: &HashMap<String, String>,
) -> Result<String, String> {
    let mut cmd = Command::new("bash");
    cmd.arg(script_path);
    // Split args by spaces respecting quotes
    for arg in shlex::split(args).unwrap_or_else(|| vec![args.to_string()]) {
        cmd.arg(arg);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.env("NUZZLE_TOOL_RUN", "1");
    for (k, v) in env_vars {
        cmd.env(k, v);
    }

    let result = timeout(Duration::from_secs(120), async {
        let output = cmd.output().await.map_err(|e| format!("Failed to execute: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut combined = String::new();
        if !stdout.trim().is_empty() {
            combined.push_str(&stdout);
        }
        if !stderr.trim().is_empty() {
            if !combined.is_empty() { combined.push_str("\n--- STDERR ---\n"); }
            combined.push_str(&stderr);
        }
        if combined.trim().is_empty() {
            combined = format!("Tool '{}' completed with no output (exit: {})",
                script_path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default(),
                output.status.code().map(|c| c.to_string()).unwrap_or_else(|| "unknown".to_string()));
        }

        Ok::<String, String>(combined)
    }).await.map_err(|_| format!("Tool timed out after 120s"))??;

    Ok(result)
}
