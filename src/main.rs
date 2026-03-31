mod args;
mod config;
mod diff;
mod parse;
mod print;
mod run;

use std::io::Write;

use anyhow::Result;
use clap::Parser;

use args::{Args, Mode};

fn main() {
    if let Err(e) = run() {
        print::print_error(&format!("{e:#}"));
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let mut out = anstream::stdout().lock();

    writeln!(out, "args loaded")?;

    write!(out, "loading config ... ")?;
    out.flush()?;
    let config = config::load(args.config_loc.as_deref())?;
    writeln!(out, "done")?;

    write!(out, "running old Oneil ... ")?;
    out.flush()?;
    let old_output = run::run_old(&config.old_repo, &config.model_file)?;
    writeln!(out, "done")?;

    write!(out, "running new Oneil ... ")?;
    out.flush()?;
    let new_output = run::run_new(&config.new_repo, &config.model_file)?;
    writeln!(out, "done")?;

    write!(out, "parsing old output ... ")?;
    out.flush()?;
    let (old_params, old_tests) = match parse::parse_old_output(&old_output) {
        Ok(result) => {
            writeln!(out, "done")?;
            result
        }
        Err(e) => {
            print::print_error(&format!("{e:#}"));
            if args.print_source_on_parse_error {
                writeln!(out, "source:")?;
                writeln!(out, "{old_output}")?;
            }
            return Ok(());
        }
    };

    write!(out, "parsing new output ... ")?;
    out.flush()?;
    let (new_params, new_tests) = match parse::parse_new_output(&new_output) {
        Ok(result) => {
            writeln!(out, "done")?;
            result
        }
        Err(e) => {
            print::print_error(&format!("{e:#}"));
            if args.print_source_on_parse_error {
                writeln!(out, "source:")?;
                writeln!(out, "{new_output}")?;
            }
            return Ok(());
        }
    };

    write!(out, "diffing params ... ")?;
    out.flush()?;
    let mut diff_params =
        diff::diff_params(&old_params, &new_params, &config.ignore_params);
    writeln!(out, "done")?;

    write!(out, "diffing tests ... ")?;
    out.flush()?;
    let mut diff_tests =
        diff::diff_tests(&old_tests, &new_tests, &config.ignore_tests);
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
