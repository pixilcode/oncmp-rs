# oncmp (Oneil Compare)

Compare parameters and regression tests between two versions of [Oneil](https://github.com/careweather/oneil): a "legacy" run using `oneil regression-test` and a "current" run using `oneil eval` and `oneil test`. Useful for validating that a refactor or new implementation produces the same parameters and test outcomes.

## Requirements

- [Rust](https://www.rust-lang.org/) (1.85+ for edition 2024)
- Two model repo checkouts: one for the old Oneil code, one for the new
- A virtualenv (`.venv`) in each repo with `oneil` and/or other Python requirements installed

## Build & install

```bash
cargo install --path .
```

Ensure `~/.cargo/bin` (or your configured Cargo install directory) is in your `PATH`.

## Usage

```text
oncmp [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `-h`, `--help` | Show help and exit |
| `--config <path>` | Path to config file (default: `./oncmp_config.toml`) |
| `-p`, `--params` | Show only parameter diffs |
| `-t`, `--tests` | Show only test diffs |
| `-i`, `--include-unchanged` | Include unchanged items in the output |
| `-e`, `--print-source-on-parse-error` | Print the raw source output when a parse error occurs |

The tool runs the old and new Oneil commands as configured, parses their output, diffs parameters and tests (respecting ignore lists), then prints the diff and a summary (added/removed/changed).

## Configuration file

Config is a single TOML file (default path: `./oncmp_config.toml`). Use `--config <path>` to override.

### Structure

```toml
[run]
old_repo = "/path/to/old/model/repo"
new_repo = "/path/to/new/model/repo"
model_file = "main_model.on"

[ignore]
params = ["param_name_to_ignore", "another_param"]
tests = ["test_name_to_ignore"]
```

### Sections and keys

| Key | Required | Description |
|-----|----------|-------------|
| **`[run]`** | | Paths and model used when invoking Oneil. |
| `run.old_repo` | Yes | Directory of the legacy model repo. The tool runs `cd <old_repo> && source .venv/bin/activate && cd model/ && oneil regression-test <model_file>`. |
| `run.new_repo` | Yes | Directory of the updated model repo. The tool runs `cd <new_repo> && source .venv/bin/activate && cd model/ && oneil eval <model_file> --print all --no-header --no-test-report && oneil test <model_file> --no-header --recursive`. |
| `run.model_file` | Yes | Model file path passed to both `oneil regression-test` (legacy) and `oneil eval` / `oneil test` (current). Relative to the `model/` subdirectory of each repo. |
| **`[ignore]`** | | Names to exclude from the diff. Useful when you have parameters or tests with changes that you have verified are correct. Omit the section or use empty arrays to diff everything. |
| `ignore.params` | No | List of parameter names to ignore when comparing. Default: `[]`. |
| `ignore.tests` | No | List of test expressions to ignore when comparing. Default: `[]`. |

### Example

```toml
[run]
old_repo = "/home/me/veery-legacy"
new_repo = "/home/me/veery-refactor"
model_file = "models/example.on"

[ignore]
params = ["already_verified_param"]
tests = ["known_flaky_test"]
```

## Crates used

| Crate | Purpose |
|-------|---------|
| [clap](https://crates.io/crates/clap) | CLI argument parsing |
| [toml](https://crates.io/crates/toml) + [serde](https://crates.io/crates/serde) | TOML config deserialization |
| [anyhow](https://crates.io/crates/anyhow) | Error handling |
| [owo-colors](https://crates.io/crates/owo-colors) | Colored terminal output |
| [anstream](https://crates.io/crates/anstream) | Auto-detection of terminal color support and ANSI stripping |
| [winnow](https://crates.io/crates/winnow) | Parser combinators for Oneil output parsing |
