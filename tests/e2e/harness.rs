use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub struct TestContext {
    pub bin_path: PathBuf,
    pub tmp_root: PathBuf,
}

pub struct TestEnv {
    pub root: PathBuf,
    pub home: PathBuf,
    pub xdg_cache: PathBuf,
    pub xdg_config: PathBuf,
}

pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl TestContext {
    pub fn new() -> Result<Self, String> {
        let bin_path = if let Some(path) = std::env::var_os("CARGO_BIN_EXE_dotdeps") {
            PathBuf::from(path)
        } else {
            let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
                .map(PathBuf::from)
                .ok_or_else(|| "CARGO_MANIFEST_DIR not set".to_string())?;
            let candidate = manifest_dir.join("target").join("debug").join("dotdeps");
            if !candidate.exists() {
                let status = Command::new("cargo")
                    .arg("build")
                    .current_dir(&manifest_dir)
                    .status()
                    .map_err(|e| format!("Failed to run cargo build: {}", e))?;
                if !status.success() {
                    return Err("cargo build failed".to_string());
                }
            }
            candidate
        };

        let tmp_root = std::env::temp_dir().join("dotdeps-e2e");
        fs::create_dir_all(&tmp_root).map_err(|e| format!("Failed to create temp root: {}", e))?;

        Ok(Self { bin_path, tmp_root })
    }

    pub fn create_env(&self, name: &str) -> Result<TestEnv, String> {
        let dir = self.unique_temp_dir(name)?;
        let home = dir.join("home");
        let xdg_cache = home.join(".cache");
        let xdg_config = home.join(".config");
        fs::create_dir_all(&xdg_cache).map_err(|e| format!("Failed to create cache dir: {}", e))?;
        fs::create_dir_all(&xdg_config)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;

        Ok(TestEnv {
            root: dir,
            home,
            xdg_cache,
            xdg_config,
        })
    }

    fn unique_temp_dir(&self, name: &str) -> Result<PathBuf, String> {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let dir = self
            .tmp_root
            .join(format!("{}-{}-{}", name, nanos, counter));
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;
        Ok(dir)
    }

    pub fn run_dotdeps(
        &self,
        env: &TestEnv,
        args: &[&str],
        cwd: &Path,
    ) -> Result<CommandOutput, String> {
        self.run_command(&self.bin_path, args, cwd, env)
    }

    pub fn run_system_command<S: AsRef<OsStr>>(
        &self,
        program: S,
        args: &[&str],
        cwd: &Path,
    ) -> Result<CommandOutput, String> {
        if std::env::var("DOTDEPS_E2E_LOG").is_ok() {
            eprintln!(
                "command: {:?} {:?} (cwd: {})",
                program.as_ref(),
                args,
                cwd.display()
            );
        }
        let output = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|e| format!("Failed to run command: {}", e))?;
        Ok(CommandOutput::from_output(output))
    }

    pub fn run_command<S: AsRef<OsStr>>(
        &self,
        program: S,
        args: &[&str],
        cwd: &Path,
        env: &TestEnv,
    ) -> Result<CommandOutput, String> {
        if std::env::var("DOTDEPS_E2E_LOG").is_ok() {
            eprintln!(
                "command: {:?} {:?} (cwd: {})",
                program.as_ref(),
                args,
                cwd.display()
            );
        }
        let output = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env("HOME", &env.home)
            .env("XDG_CACHE_HOME", &env.xdg_cache)
            .env("XDG_CONFIG_HOME", &env.xdg_config)
            .output()
            .map_err(|e| format!("Failed to run command: {}", e))?;

        Ok(CommandOutput::from_output(output))
    }

    pub fn command_available(&self, program: &str) -> bool {
        Command::new(program).arg("--version").output().is_ok()
    }
}

impl CommandOutput {
    pub fn from_output(output: Output) -> Self {
        let status = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Self {
            status,
            stdout,
            stderr,
        }
    }

    pub fn assert_success(&self) -> Result<(), String> {
        if self.status == 0 {
            Ok(())
        } else {
            Err(format!(
                "Expected success, got exit {}: {}",
                self.status, self.stderr
            ))
        }
    }

    pub fn assert_failure(&self) -> Result<(), String> {
        if self.status != 0 {
            Ok(())
        } else {
            Err("Expected failure, got success".to_string())
        }
    }

    pub fn assert_stdout_contains(&self, needle: &str) -> Result<(), String> {
        if self.stdout.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "Expected stdout to contain '{}'.\nstdout: {}",
                needle, self.stdout
            ))
        }
    }

    pub fn assert_stdout_not_contains(&self, needle: &str) -> Result<(), String> {
        if !self.stdout.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "Expected stdout to not contain '{}'.\nstdout: {}",
                needle, self.stdout
            ))
        }
    }

    pub fn assert_stderr_contains(&self, needle: &str) -> Result<(), String> {
        if self.stderr.contains(needle) {
            Ok(())
        } else {
            Err(format!(
                "Expected stderr to contain '{}'.\nstderr: {}",
                needle, self.stderr
            ))
        }
    }
}

pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dirs: {}", e))?;
    }
    fs::write(path, content).map_err(|e| format!("Failed to write file: {}", e))
}

pub fn read_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))
}

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("Failed to create dir: {}", e))
}

pub fn symlink_dir(src: &Path, dst: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst)
            .map_err(|e| format!("Failed to create symlink: {}", e))?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(src, dst)
            .map_err(|e| format!("Failed to create symlink: {}", e))?;
    }
    Ok(())
}

pub fn parse_json(output: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(output).map_err(|e| format!("Invalid JSON output: {}", e))
}
