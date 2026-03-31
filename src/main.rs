mod args;
mod config;
mod diff;
mod parse;
mod print;
mod run;

use std::io::Write;
use std::thread;

use anyhow::Result;
use clap::Parser;

use args::{Args, Mode};

fn main() -> Result<()> {
    let args = Args::parse();
    let mut out = anstream::stdout().lock();

    writeln!(out, "args loaded")?;

    write!(out, "loading config ... ")?;
    out.flush()?;
    let config = config::load(args.config_loc.as_deref())?;
    writeln!(out, "done")?;

    // Run old and new Oneil commands in parallel
    writeln!(out, "running old and new Oneil ...")?;
    out.flush()?;
    let (old_output, new_output) = thread::scope(|s| {
        let old_handle = s.spawn(|| run::run_old(&config.old_repo, &config.model_file));
        let new_handle = s.spawn(|| run::run_new(&config.new_repo, &config.model_file));
        anyhow::Ok((old_handle.join().unwrap()?, new_handle.join().unwrap()?))
    })?;
    writeln!(out, "done")?;

    // Parse both outputs in parallel
    writeln!(out, "parsing outputs ...")?;
    out.flush()?;
    let (old_parsed, new_parsed) = thread::scope(|s| {
        let old_handle = s.spawn(|| parse::parse_old_output(&old_output));
        let new_handle = s.spawn(|| parse::parse_new_output(&new_output));
        (old_handle.join().unwrap(), new_handle.join().unwrap())
    });

    let (old_params, old_tests) = match old_parsed {
        Ok(result) => result,
        Err(e) => {
            print::print_error(&format!("{e:#}"));
            if args.print_source_on_parse_error {
                writeln!(out, "source:")?;
                writeln!(out, "{old_output}")?;
            }
            return Ok(());
        }
    };

    let (new_params, new_tests) = match new_parsed {
        Ok(result) => result,
        Err(e) => {
            print::print_error(&format!("{e:#}"));
            if args.print_source_on_parse_error {
                writeln!(out, "source:")?;
                writeln!(out, "{new_output}")?;
            }
            return Ok(());
        }
    };
    writeln!(out, "done")?;

    // Diff params and tests in parallel
    writeln!(out, "diffing ...")?;
    out.flush()?;
    let (mut diff_params, mut diff_tests) = thread::scope(|s| {
        let params_handle =
            s.spawn(|| diff::diff_params(&old_params, &new_params, &config.ignore_params));
        let tests_handle =
            s.spawn(|| diff::diff_tests(&old_tests, &new_tests, &config.ignore_tests));
        (params_handle.join().unwrap(), tests_handle.join().unwrap())
    });
    writeln!(out, "done")?;

    writeln!(out)?;

    let include_unchanged = args.include_unchanged;
    match args.mode() {
        Mode::All => {
            print_params(&mut out, &mut diff_params, include_unchanged)?;
            print_tests(&mut out, &mut diff_tests, include_unchanged)?;
        }
        Mode::Params => {
            print_params(&mut out, &mut diff_params, include_unchanged)?;
        }
        Mode::Tests => {
            print_tests(&mut out, &mut diff_tests, include_unchanged)?;
        }
    }

    Ok(())
}

fn print_params(
    out: &mut impl Write,
    diffs: &mut [diff::Diff<parse::Param>],
    include_unchanged: bool,
) -> Result<()> {
    writeln!(out, "========== PARAMETERS ==========")?;
    print::print_params_diff(out, diffs, include_unchanged);
    writeln!(out)?;
    print::print_diff_summary(out, diffs);
    writeln!(out)?;
    Ok(())
}

fn print_tests(
    out: &mut impl Write,
    diffs: &mut [diff::Diff<parse::Test>],
    include_unchanged: bool,
) -> Result<()> {
    writeln!(out, "========== TESTS ==========")?;
    print::print_tests_diff(out, diffs, include_unchanged);
    writeln!(out)?;
    print::print_diff_summary(out, diffs);
    Ok(())
}
