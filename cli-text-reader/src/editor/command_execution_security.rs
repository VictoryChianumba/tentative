use std::collections::HashSet;
use std::process::{Command, Stdio};
use std::time::Duration;

// Command output structure
pub struct CommandOutput {
  pub stdout: String,
  pub stderr: String,
  pub status: std::process::ExitStatus,
}

// Secure command structure for validated commands
#[derive(Debug)]
pub struct SecureCommand {
  pub program: String,
  pub args: Vec<String>,
}

// Parse and validate command using whitelist approach
pub fn parse_secure_command(cmd: &str) -> Result<SecureCommand, String> {
  let cmd = cmd.trim();
  if cmd.is_empty() {
    return Err("Empty command".to_string());
  }

  let cmd_to_parse = cmd;

  // Whitelist of allowed commands — read-only filesystem and text operations only.
  // Excluded intentionally:
  //   env/printenv/history — expose secrets from the process environment and shell history
  //   curl/wget/ping/dig/nslookup — make outbound network connections
  //   tar/zip/unzip/gzip/gunzip/zcat — can write arbitrary files during extraction
  //   echo/printf — can be chained with redirects to write files in some contexts
  //   PowerShell entries — Windows translation is a separate concern; do not expand attack surface here
  let allowed_commands: HashSet<&str> = [
    // Directory listing and path navigation
    "ls",
    "pwd",
    "find",
    "locate",
    "which",
    "whereis",
    // File viewing (core functionality for text reader)
    "cat",
    "less",
    "more",
    "head",
    "tail",
    "file",
    "stat",
    "wc",
    "nl",
    // Text processing (read-only)
    "grep",
    "awk",
    "sed",
    "sort",
    "uniq",
    "cut",
    "tr",
    "fmt",
    "fold",
    // System information (read-only, no secrets)
    "date",
    "uptime",
    "whoami",
    "id",
    "uname",
    "hostname",
    "df",
    "free",
    "ps",
    // Path utilities
    "basename",
    "dirname",
    "realpath",
    "readlink",
  ]
  .iter()
  .cloned()
  .collect();

  // Shell-quote-aware tokenisation so filenames with spaces work correctly.
  let parts: Vec<String> = shlex::split(cmd_to_parse)
    .ok_or_else(|| "Malformed quoting in command".to_string())?;
  if parts.is_empty() {
    return Err("Invalid command".to_string());
  }

  let program = &parts[0];

  // Check if command is whitelisted
  if !allowed_commands.contains(program.as_str()) {
    return Err(format!("Command '{program}' is not allowed"));
  }

  // Reject shell metacharacters in the post-split tokens.
  let dangerous_chars: &[char] =
    &['|', '&', ';', '`', '$', '(', ')', '<', '>', '\\', '*', '?'];

  for arg in &parts[1..] {
    if arg.chars().any(|c| dangerous_chars.contains(&c)) {
      return Err(format!("Argument contains dangerous characters: {arg}"));
    }
    if arg.len() > 1000 {
      return Err("Argument too long (max 1000 characters)".to_string());
    }
  }

  if parts.len() > 50 {
    return Err("Too many arguments (max 50)".to_string());
  }

  Ok(SecureCommand {
    program: program.clone(),
    args: parts[1..].to_vec(),
  })
}

// Execute a validated command with timeout.
//
// Spawns a dedicated thread that does the blocking wait, then uses
// `recv_timeout` to implement the deadline without polling.
pub fn execute_secure_command_with_timeout(
  secure_cmd: SecureCommand,
  timeout: Duration,
) -> Result<CommandOutput, String> {
  use std::io::Read;
  use std::sync::mpsc;

  let mut child = Command::new(&secure_cmd.program)
    .args(&secure_cmd.args)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| {
      format!("Failed to execute command '{}': {}", secure_cmd.program, e)
    })?;

  // Take ownership of the stdio handles before handing off the child.
  let mut stdout_handle = child.stdout.take();
  let mut stderr_handle = child.stderr.take();

  let (tx, rx) = mpsc::channel::<std::io::Result<std::process::ExitStatus>>();
  std::thread::spawn(move || {
    let _ = tx.send(child.wait());
  });

  match rx.recv_timeout(timeout) {
    Ok(Ok(status)) => {
      let mut stdout_bytes = Vec::new();
      let mut stderr_bytes = Vec::new();
      if let Some(ref mut h) = stdout_handle {
        let _ = h.read_to_end(&mut stdout_bytes);
      }
      if let Some(ref mut h) = stderr_handle {
        let _ = h.read_to_end(&mut stderr_bytes);
      }
      Ok(CommandOutput {
        stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
        status,
      })
    }
    Ok(Err(e)) => Err(format!("Failed to wait for command: {e}")),
    Err(mpsc::RecvTimeoutError::Timeout) => {
      Err(format!("Command timed out after {} seconds", timeout.as_secs()))
    }
    Err(mpsc::RecvTimeoutError::Disconnected) => {
      Err("Command wait thread disconnected unexpectedly".to_string())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::parse_secure_command;

  #[test]
  fn test_allowed_commands() {
    let allowed = vec!["cat", "less", "head", "tail", "grep", "ls", "pwd"];
    for cmd in allowed {
      assert!(parse_secure_command(cmd).is_ok(), "{cmd} should be allowed");
    }
  }

  #[test]
  fn test_rejected_commands() {
    let rejected = vec!["rm", "sudo", "kill", "reboot"];
    for cmd in rejected {
      assert!(parse_secure_command(cmd).is_err(), "{cmd} should be rejected");
    }
  }

  #[test]
  fn test_dangerous_chars() {
    let dangerous =
      vec!["cat file; rm file", "echo `cmd`", "ls > file", "cmd | other"];
    for input in dangerous {
      assert!(
        parse_secure_command(input).is_err(),
        "{input} should be rejected"
      );
    }
  }

  #[test]
  fn test_quoted_filenames() {
    let result = parse_secure_command("cat 'my file.txt'").unwrap();
    assert_eq!(result.program, "cat");
    assert_eq!(result.args, vec!["my file.txt"]);

    let result = parse_secure_command(r#"cat "my file.txt""#).unwrap();
    assert_eq!(result.program, "cat");
    assert_eq!(result.args, vec!["my file.txt"]);
  }
}
