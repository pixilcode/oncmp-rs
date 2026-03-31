use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn run_old(old_repo: &str, model_file: &str) -> Result<String> {
    let command = format!(
        "cd {old_repo} && source .venv/bin/activate && cd model/ && oneil regression-test {model_file}"
    );
    run_command(&command, old_repo)
}

pub fn run_new(new_repo: &str, model_file: &str) -> Result<String> {
    let command = format!(
        "cd {new_repo} && source .venv/bin/activate && cd model/ && \
         oneil eval {model_file} --print all --no-header --no-test-report && \
         oneil test {model_file} --no-header --recursive"
    );
    run_command(&command, new_repo)
}

fn run_command(command: &str, location: &str) -> Result<String> {
    let output = Command::new("sh")
        .args(["-c", command])
        .current_dir(location)
        .output()
        .with_context(|| format!("failed to spawn command: {command}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "command failed (exit code: {}):\n{stdout}\n{stderr}\nCommand: {command}\nLocation: {location}",
            output.status.code().unwrap_or(-1)
        );
    }

    let raw = String::from_utf8(output.stdout)
        .with_context(|| format!("non-UTF-8 output from command: {command}"))?;

    Ok(anstream::adapter::strip_str(&raw).to_string())
}
