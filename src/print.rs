use std::io::Write;

use owo_colors::OwoColorize;

use crate::diff::Diff;
use crate::parse::{Param, ParamValue, Test, TestDependencyParam, TestResult};

const INDENT_AMOUNT: usize = 2;

pub fn print_error(error: &str) {
    let mut stderr = anstream::stderr();
    let msg = format!("\nerror: {error}");
    writeln!(stderr, "{}", msg.red()).ok();
}

struct DiffSummary {
    added: usize,
    removed: usize,
    changed: usize,
}

pub fn print_diff_summary<T>(out: &mut impl Write, diffs: &[Diff<T>]) {
    let summary = diffs.iter().fold(
        DiffSummary {
            added: 0,
            removed: 0,
            changed: 0,
        },
        |acc, diff| match diff {
            Diff::OldOnly(_) => DiffSummary {
                removed: acc.removed + 1,
                ..acc
            },
            Diff::Same(_) => acc,
            Diff::Different(_, _) => DiffSummary {
                changed: acc.changed + 1,
                ..acc
            },
            Diff::NewOnly(_) => DiffSummary {
                added: acc.added + 1,
                ..acc
            },
        },
    );

    writeln!(
        out,
        "{} {} added, {} removed, {} changed",
        "summary:".bold(),
        summary.added,
        summary.removed,
        summary.changed,
    )
    .ok();
}

pub fn print_params_diff(out: &mut impl Write, diffs: &mut [Diff<Param>], include_unchanged: bool) {
    diffs.sort_by(|a, b| a.first().name.cmp(&b.first().name));
    print_diff(out, diffs, include_unchanged, param_to_string);
}

pub fn print_tests_diff(out: &mut impl Write, diffs: &mut [Diff<Test>], include_unchanged: bool) {
    diffs.sort_by(|a, b| {
        let (a, b) = (a.first(), b.first());
        a.model
            .cmp(&b.model)
            .then_with(|| a.expression.cmp(&b.expression))
    });
    print_diff(out, diffs, include_unchanged, test_to_string);
}

fn print_diff<T>(
    out: &mut impl Write,
    diffs: &[Diff<T>],
    include_unchanged: bool,
    to_string: fn(&T) -> String,
) {
    for diff in diffs {
        match diff {
            Diff::OldOnly(a) => {
                let text = indent_all_with_prefix(&to_string(a), INDENT_AMOUNT, "-");
                writeln!(out, "{}", text.red()).ok();
            }
            Diff::Same(a) if include_unchanged => {
                let text = indent_all_with_prefix(&to_string(a), INDENT_AMOUNT, " ");
                writeln!(out, "{text}").ok();
            }
            Diff::Same(_) => {}
            Diff::Different(old, new) => {
                let old_text = indent_all_with_prefix(&to_string(old), INDENT_AMOUNT, "-");
                writeln!(out, "{}", old_text.red()).ok();
                let new_text = indent_all_with_prefix(&to_string(new), INDENT_AMOUNT, "+");
                writeln!(out, "{}", new_text.green()).ok();
            }
            Diff::NewOnly(a) => {
                let text = indent_all_with_prefix(&to_string(a), INDENT_AMOUNT, "+");
                writeln!(out, "{}", text.green()).ok();
            }
        }
    }
}

fn param_to_string(param: &Param) -> String {
    let value = value_to_string(&param.value);
    let unit = param
        .unit
        .as_ref()
        .map(|u| format!(" :{u}"))
        .unwrap_or_default();
    format!("{} = {value}{unit}  # {}", param.name, param.description)
}

fn test_to_string(test: &Test) -> String {
    let main_line = format!("test ({}): {}", test.model, test.expression);

    let (result_line, param_lines) = match &test.result {
        TestResult::Pass => (indent_line("result: pass", INDENT_AMOUNT), vec![]),
        TestResult::Fail(params) => {
            let mut sorted = params.clone();
            sorted.sort_by(|a, b| a.name.cmp(&b.name));
            let lines = sorted
                .iter()
                .map(|p| indent_line(&test_dependency_param_to_string(p), INDENT_AMOUNT))
                .collect();
            (indent_line("result: fail", INDENT_AMOUNT), lines)
        }
    };

    let mut parts = vec![main_line, result_line];
    parts.extend(param_lines);
    parts.join("\n")
}

fn test_dependency_param_to_string(param: &TestDependencyParam) -> String {
    let value = value_to_string(&param.value);
    let unit = param
        .unit
        .as_ref()
        .map(|u| format!(" :{u}"))
        .unwrap_or_default();
    format!("- {} = {value}{unit}", param.name)
}

fn value_to_string(value: &ParamValue) -> String {
    match value {
        ParamValue::Scalar(v) => format_float(*v),
        ParamValue::EmptyInterval => "<empty>".to_string(),
        ParamValue::Interval { min, max } => {
            format!("{} | {}", format_float(*min), format_float(*max))
        }
        ParamValue::Str(v) => format!("'{v}'"),
    }
}

fn format_float(v: f64) -> String {
    let s = v.to_string();
    if s.contains('.') { s } else { format!("{s}.0") }
}

fn indent_line(text: &str, amount: usize) -> String {
    format!("{:amount$}{text}", "")
}

fn indent_all_with_prefix(text: &str, amount: usize, prefix: &str) -> String {
    let padding = amount.saturating_sub(prefix.len());
    let indent = format!("{prefix}{:padding$}", "");
    text.lines()
        .map(|line| format!("{indent}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
