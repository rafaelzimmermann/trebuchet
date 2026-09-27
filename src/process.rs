//! Captured command results retain process status and both output streams.

pub struct CommandOutput {
    pub status: std::process::ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if !self.status.success() {
            parts.push(format!("Command failed ({})", self.status));
        }
        if !self.stdout.is_empty() {
            parts.push(self.stdout.clone());
        }
        if !self.stderr.is_empty() {
            parts.push(self.stderr.clone());
        }
        if parts.is_empty() {
            "(no output)".into()
        } else {
            parts.join("\n")
        }
    }
}

pub async fn run_command(command: &str) -> Result<CommandOutput, String> {
    let output = tokio::process::Command::new("sh")
        .args(["-c", command])
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("Could not run command: {e}"))?;
    Ok(CommandOutput {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn preserves_failure_status_and_both_streams() {
        let output = run_command("printf partial; printf 'failure detail' >&2; exit 7")
            .await
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout, "partial");
        assert_eq!(output.stderr, "failure detail");
        assert!(output.display().contains("Command failed"));
        assert!(output.display().contains("failure detail"));
    }
    #[tokio::test]
    async fn successful_empty_output_and_stderr_are_distinct() {
        assert_eq!(run_command("true").await.unwrap().display(), "(no output)");
        assert_eq!(
            run_command("printf warning >&2").await.unwrap().display(),
            "warning"
        );
    }
}
