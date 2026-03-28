//! Process execution and I/O primitives with timeout and cleanup.
//!
//! Handles spawning gcc, draining stdout/stderr, process timeout, and zombie cleanup.

use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::constants::{GCC_BINARY, GCC_FLAGS, MAX_OUTPUT_BYTES};
use crate::error::KfError;

/// Prefix used in timeout error messages — also used for pattern matching.
pub const TIMEOUT_MSG_PREFIX: &str = "Délai d'exécution dépassé";

/// Wait for a child process to complete or timeout, handling process group termination.
/// Returns the exit status if successful, or an error if timeout or wait fails.
pub fn wait_for_process_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> crate::error::Result<std::process::ExitStatus> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    kill_process_group(child);
                    if let Err(e) = child.wait() {
                        if e.raw_os_error() != Some(libc::ECHILD) {
                            eprintln!("[clings/runner] avertissement : reap zombie échoué : {e}");
                        }
                    }
                    return Err(KfError::Config(format!(
                        "{TIMEOUT_MSG_PREFIX} ({:.1}s limite)",
                        timeout.as_secs_f64()
                    )));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => {
                kill_process_group(child);
                if let Err(we) = child.wait() {
                    if we.raw_os_error() != Some(libc::ECHILD) {
                        eprintln!("[clings/runner] avertissement : reap zombie échoué : {we}");
                    }
                }
                return Err(KfError::Io(e));
            }
        }
    }
}

/// Collect stdout and stderr from drain threads, converting panics to errors.
pub fn drain_stdio(
    stdout_thread: std::thread::JoinHandle<String>,
    stderr_thread: std::thread::JoinHandle<String>,
) -> crate::error::Result<(String, String)> {
    let stdout = stdout_thread
        .join()
        .map_err(|_| KfError::Config("stdout reader thread paniqué".to_owned()))?;
    let stderr = stderr_thread
        .join()
        .map_err(|_| KfError::Config("stderr reader thread paniqué".to_owned()))?;
    Ok((stdout, stderr))
}

/// Compile `source_path` with gcc `extra_args`, run the resulting binary from
/// `work_dir`, and collect stdout + stderr within `timeout`.
/// Returns `(stdout, stderr, exit_status)` or a `KfError`.
pub fn spawn_gcc_and_collect(
    source_path: &Path,
    extra_args: &[&str],
    work_dir: &Path,
    timeout: Duration,
) -> crate::error::Result<(String, String, std::process::ExitStatus)> {
    let mut gcc = Command::new(GCC_BINARY);
    gcc.args(GCC_FLAGS);
    for arg in extra_args {
        gcc.arg(arg);
    }

    let compile_result = gcc.output().map_err(|e| {
        KfError::Io(std::io::Error::new(
            e.kind(),
            format!("Impossible de lancer gcc : {e}. gcc est-il installé ?"),
        ))
    })?;

    if !compile_result.status.success() {
        let stderr = String::from_utf8_lossy(&compile_result.stderr).to_string();
        return Err(KfError::Config(stderr));
    }

    let output_path = extra_args
        .windows(2)
        .find(|w| w[0] == "-o")
        .ok_or_else(|| KfError::Config("extra_args must contain -o <output>".to_string()))
        .map(|w| std::path::PathBuf::from(w[1]))?;

    let _ = source_path;

    let mut child = Command::new(&output_path)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(KfError::Io)?;

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            kill_process_group(&child);
            return Err(KfError::Config("stdout pipe non disponible".to_owned()));
        }
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            kill_process_group(&child);
            return Err(KfError::Config("stderr pipe non disponible".to_owned()));
        }
    };
    let (stdout_thread, stderr_thread) = spawn_drain_threads(stdout, stderr);
    let status = wait_for_process_with_timeout(&mut child, timeout)?;
    let (stdout, stderr) = drain_stdio(stdout_thread, stderr_thread)?;
    Ok((stdout, stderr, status))
}

/// Spawn background threads to drain stdout and stderr from a child process.
/// Returns (stdout_thread, stderr_thread) handles so the caller can join them.
pub fn spawn_drain_threads(
    stdout: std::process::ChildStdout,
    stderr: std::process::ChildStderr,
) -> (
    std::thread::JoinHandle<String>,
    std::thread::JoinHandle<String>,
) {
    let stdout_thread = std::thread::spawn(move || -> String {
        let mut buf = String::new();
        if let Err(e) = Read::read_to_string(&mut Read::take(stdout, MAX_OUTPUT_BYTES), &mut buf) {
            eprintln!("[clings/runner] avertissement : lecture pipe stdout : {e}");
        }
        buf
    });
    let stderr_thread = std::thread::spawn(move || -> String {
        let mut buf = String::new();
        if let Err(e) = Read::read_to_string(&mut Read::take(stderr, MAX_OUTPUT_BYTES), &mut buf) {
            eprintln!("[clings/runner] avertissement : lecture pipe stderr : {e}");
        }
        buf
    });
    (stdout_thread, stderr_thread)
}

/// Kill the entire process group of a child to avoid zombie fork-bombs.
pub fn kill_process_group(child: &std::process::Child) {
    let pid = child.id();
    if pid == 0 {
        return;
    }
    // SAFETY: libc::kill is called with a valid process group (negated PID, checked > 0 above)
    // and a valid signal constant (SIGKILL). Return value is ignored as we can't meaningfully
    // handle errors in a cleanup context. Process is already terminating.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// drain_stdio: threads produce expected strings, drain_stdio returns them correctly.
    #[test]
    fn test_drain_stdio_returns_thread_output() {
        let stdout_thread = std::thread::spawn(|| "hello stdout".to_string());
        let stderr_thread = std::thread::spawn(|| "hello stderr".to_string());
        let result = drain_stdio(stdout_thread, stderr_thread);
        assert!(result.is_ok(), "drain_stdio should succeed");
        let (stdout, stderr) = result.unwrap();
        assert_eq!(stdout, "hello stdout");
        assert_eq!(stderr, "hello stderr");
    }

    /// drain_stdio: handles empty strings from both threads.
    #[test]
    fn test_drain_stdio_empty_output() {
        let stdout_thread = std::thread::spawn(|| String::new());
        let stderr_thread = std::thread::spawn(|| String::new());
        let (stdout, stderr) = drain_stdio(stdout_thread, stderr_thread).unwrap();
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }
}
