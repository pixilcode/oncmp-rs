use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn run_old(old_repo: &str, model_file: &str) -> Result<String> {
    let command = format!(
        "cd {old_repo} && source .venv/bin/activate && cd model/ && oneil regression-test {model_file}"
    );
    run_command(&command, old_repo).context("running old Oneil")
}

pub fn run_new(new_repo: &str, model_file: &str) -> Result<String> {
    let command = format!(
        "cd {new_repo} && source .venv/bin/activate && cd model/ && \
         oneil eval {model_file} --print all --no-header --no-test-report && \
         oneil test {model_file} --no-header --recursive"
    );
    run_command(&command, new_repo).context("running new Oneil")
}

fn run_command(command: &str, location: &str) -> Result<String> {
    let output = Command::new("sh")
        .args(["-c", command])
        .current_dir(location)
        .output()
        .with_context(|| format!("failed to spawn shell in {location:?}"))?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut msg = format!("command exited with code {code}");
        msg.push_str(&format!("\n  command: {command}"));
        msg.push_str(&format!("\n  directory: {location}"));
        if !stderr.trim().is_empty() {
            msg.push_str(&format!("\n  stderr:\n    {}", stderr.trim().replace('\n', "\n    ")));
        }
        if !stdout.trim().is_empty() {
            msg.push_str(&format!("\n  stdout:\n    {}", stdout.trim().replace('\n', "\n    ")));
        }
        bail!("{msg}");
    }

    let raw = String::from_utf8(output.stdout)
        .context("command produced non-UTF-8 output")?;

    Ok(anstream::adapter::strip_str(&raw).to_string())
}
