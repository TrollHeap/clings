use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::models::{Exercise, ValidationMode};

pub struct RunResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub compile_error: bool,
    pub timeout: bool,
}

/// Determine linker flags based on subject.
fn linker_flags(subject: &str) -> Vec<&'static str> {
    match subject {
        "pthreads" | "semaphores" | "sync_concepts" | "sockets" => vec!["-lpthread"],
        "message_queues" | "shared_memory" => vec!["-lrt", "-lpthread"],
        _ => vec![],
    }
}

/// Write exercise files (headers etc.) to a temp directory.
fn write_exercise_files(exercise: &Exercise, work_dir: &Path) -> std::io::Result<()> {
    for file in &exercise.files {
        let file_path = work_dir.join(&file.name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, &file.content)?;
    }
    Ok(())
}

/// Compile and run a C source file, validating output against expected.
pub fn compile_and_run(source_path: &Path, exercise: &Exercise) -> RunResult {
    let work_dir = source_path.parent().unwrap_or(Path::new("/tmp"));
    let output_path = work_dir.join("kf_run");

    // Write additional files (headers etc.)
    if let Err(e) = write_exercise_files(exercise, work_dir) {
        return RunResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Failed to write exercise files: {e}"),
            duration_ms: 0,
            compile_error: true,
            timeout: false,
        };
    }

    // Compile
    let mut gcc = Command::new("gcc");
    gcc.args(["-Wall", "-Wextra", "-std=c11"])
        .arg("-o")
        .arg(&output_path)
        .arg(source_path);

    // Add include path for headers
    gcc.arg(format!("-I{}", work_dir.display()));

    // Add linker flags
    for flag in linker_flags(&exercise.subject) {
        gcc.arg(flag);
    }

    let compile_result = match gcc.output() {
        Ok(r) => r,
        Err(e) => {
            return RunResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to run gcc: {e}. Is gcc installed?"),
                duration_ms: 0,
                compile_error: true,
                timeout: false,
            };
        }
    };

    if !compile_result.status.success() {
        return RunResult {
            success: false,
            stdout: String::new(),
            stderr: String::from_utf8_lossy(&compile_result.stderr).to_string(),
            duration_ms: 0,
            compile_error: true,
            timeout: false,
        };
    }

    // Execute with timeout
    let start = Instant::now();
    let child = Command::new(&output_path)
        .current_dir(work_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to execute: {e}"),
                duration_ms: 0,
                compile_error: false,
                timeout: false,
            };
        }
    };

    let timeout = Duration::from_secs(10);
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let stdout = child
                .stdout
                .map(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok();
                    buf
                })
                .unwrap_or_default();
            let stderr = child
                .stderr
                .map(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok();
                    buf
                })
                .unwrap_or_default();

            if !status.success() {
                return RunResult {
                    success: false,
                    stdout,
                    stderr: if stderr.is_empty() {
                        format!("Process exited with {status}")
                    } else {
                        stderr
                    },
                    duration_ms,
                    compile_error: false,
                    timeout: false,
                };
            }

            // Validate output
            let valid = validate_output(&stdout, exercise);
            RunResult {
                success: valid,
                stdout,
                stderr,
                duration_ms,
                compile_error: false,
                timeout: false,
            }
        }
        Ok(None) => {
            // Timeout — kill the process group
            let _ = child.kill();
            let _ = child.wait();
            RunResult {
                success: false,
                stdout: String::new(),
                stderr: "Execution timed out (10s limit)".to_string(),
                duration_ms: 10_000,
                compile_error: false,
                timeout: true,
            }
        }
        Err(e) => RunResult {
            success: false,
            stdout: String::new(),
            stderr: format!("Wait error: {e}"),
            duration_ms: start.elapsed().as_millis() as u64,
            compile_error: false,
            timeout: false,
        },
    }
}

/// Validate program output against expected output.
fn validate_output(stdout: &str, exercise: &Exercise) -> bool {
    match exercise.validation.mode {
        ValidationMode::Output => {
            if let Some(expected) = &exercise.validation.expected_output {
                normalize(stdout) == normalize(expected)
            } else {
                // No expected output defined — just check it compiled and ran
                true
            }
        }
        ValidationMode::Test | ValidationMode::Both => {
            // Test mode not supported in CLI MVP — warn
            false
        }
    }
}

/// Normalize output: trim, normalize newlines, remove trailing whitespace per line.
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n")
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Get the working directory for exercises.
pub fn work_dir() -> PathBuf {
    let dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".kernelforge");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Map mastery score to stage index (0-4).
pub fn mastery_to_stage(mastery: f64) -> u8 {
    match mastery {
        m if m < 1.0 => 0,
        m if m < 2.0 => 1,
        m if m < 3.0 => 2,
        m if m < 4.0 => 3,
        _ => 4,
    }
}

/// Select the appropriate starter code stage based on mastery score.
/// Higher mastery → harder stage (less scaffolding).
pub fn select_starter_code(exercise: &Exercise, mastery: f64) -> &str {
    let stage = mastery_to_stage(mastery) as usize;
    exercise
        .starter_code_stages
        .get(stage)
        .map(|s| s.as_str())
        .unwrap_or(&exercise.starter_code)
}

/// Write starter code to the current.c file.
/// If mastery is provided, selects the appropriate stage.
pub fn write_starter_code(exercise: &Exercise, mastery: Option<f64>) -> std::io::Result<PathBuf> {
    let dir = work_dir();
    let source_path = dir.join("current.c");
    let code = match mastery {
        Some(m) => select_starter_code(exercise, m),
        None => &exercise.starter_code,
    };
    let mut f = std::fs::File::create(&source_path)?;
    f.write_all(code.as_bytes())?;

    // Write additional files
    write_exercise_files(exercise, &dir)?;

    Ok(source_path)
}

/// Trait to add wait_timeout to Child (not in std).
trait ChildExt {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>>;
}

impl ChildExt for std::process::Child {
    fn wait_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<std::process::ExitStatus>> {
        let start = Instant::now();
        loop {
            match self.try_wait()? {
                Some(status) => return Ok(Some(status)),
                None => {
                    if start.elapsed() >= timeout {
                        return Ok(None);
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }
}
