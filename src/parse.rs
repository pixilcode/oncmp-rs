use anyhow::{Context, Result, bail};
use winnow::ascii::space0;
use winnow::prelude::*;
use winnow::token::{rest, take_until};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub value: ParamValue,
    pub unit: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone)]
pub enum ParamValue {
    Scalar(f64),
    Interval { min: f64, max: f64 },
    EmptyInterval,
    Str(String),
}

#[derive(Debug, Clone)]
pub struct Test {
    pub model: String,
    pub expression: String,
    pub result: TestResult,
}

#[derive(Debug, Clone)]
pub enum TestResult {
    Pass,
    Fail(Vec<TestDependencyParam>),
}

#[derive(Debug, Clone)]
pub struct TestDependencyParam {
    pub name: String,
    pub value: ParamValue,
    pub unit: Option<String>,
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// ---------------------------------------------------------------------------
// Shared value parsing
// ---------------------------------------------------------------------------

fn parse_int_or_float(input: &str) -> Option<f64> {
    let input = input.trim();

    // Handle scientific notation without a decimal (e.g. "1e5" -> "1.0e5")
    let normalized = if input.contains('e') && !input.contains('.') {
        input.replace("e", ".0e")
    } else {
        input.to_string()
    };

    normalized
        .trim()
        .parse::<f64>()
        .ok()
        .or_else(|| input.parse::<i64>().ok().map(|v| v as f64))
}

fn parse_param_value(input: &str) -> Result<ParamValue> {
    let input = input.trim();

    if input == "<empty>" {
        return Ok(ParamValue::EmptyInterval);
    }

    match input.split_once('|') {
        Some((min_s, max_s)) => {
            let min = parse_int_or_float(min_s);
            let max = parse_int_or_float(max_s);
            match (min, max) {
                (Some(min), Some(max)) => Ok(ParamValue::Interval { min, max }),
                (None, None) => Ok(ParamValue::Str(input.to_string())),
                _ => bail!(
                    "invalid interval (one side is numeric, the other is not): \
                     min={min_s:?}, max={max_s:?}"
                ),
            }
        }
        None => parse_int_or_float(input)
            .map(ParamValue::Scalar)
            .ok_or(())
            .or_else(|()| Ok(ParamValue::Str(parse_string(input)))),
    }
}

fn parse_string(value: &str) -> String {
    value.trim().replace("'", "")
}

// ---------------------------------------------------------------------------
// Value + optional unit helpers (shared between old and new parsers)
// ---------------------------------------------------------------------------

fn split_value_unit(text: &str, separator: &str) -> (String, Option<String>) {
    text.split_once(separator)
        .map(|(v, u)| (v.trim().to_string(), Some(u.trim().to_string())))
        .unwrap_or_else(|| (text.trim().to_string(), None))
}

// ---------------------------------------------------------------------------
// Old output parsing
// ---------------------------------------------------------------------------

/// Parse the old Oneil output format (from `oneil regression-test`).
pub fn parse_old_output(output: &str) -> Result<(Vec<Param>, Vec<Test>)> {
    let params = output
        .lines()
        .filter(|line| line.contains("-- \""))
        .enumerate()
        .map(|(i, line)| {
            parse_old_param(line)
                .with_context(|| format!("old param #{} (line: {:?})", i + 1, truncate(line, 80)))
        })
        .collect::<Result<Vec<_>>>()
        .context("failed to parse old output parameters")?;

    let tests = output
        .split("\nTest (")
        .skip(1)
        .enumerate()
        .map(|(i, chunk)| {
            parse_old_test(chunk).with_context(|| {
                format!(
                    "old test #{} (starts with: {:?})",
                    i + 1,
                    truncate(chunk.trim(), 60)
                )
            })
        })
        .collect::<Result<Vec<_>>>()
        .context("failed to parse old output tests")?;

    Ok((params, tests))
}

fn parse_old_param(line: &str) -> Result<Param> {
    fn inner(input: &mut &str) -> ModalResult<Param> {
        let name = take_until(1.., ":").parse_next(input)?;
        let name = name.trim().to_string();
        ":".parse_next(input)?;

        space0.parse_next(input)?;
        let value_str: &str = take_until(1.., " ").parse_next(input)?;
        " ".parse_next(input)?;

        let checkpoint = *input;
        let (value, remaining) =
            if let Some(after) = input.strip_prefix(&format!("| {value_str}")) {
                *input = after;
                (ParamValue::Str(value_str.to_string()), *input)
            } else {
                *input = checkpoint;
                let value = parse_param_value(value_str)
                    .map_err(|_| winnow::error::ErrMode::from_input(input))?;
                (value, *input)
            };

        let _ = remaining;

        let before_desc: &str = take_until(1.., "-- \"").parse_next(input)?;
        "-- \"".parse_next(input)?;

        let unit = match before_desc.trim() {
            "" => None,
            u => Some(u.to_string()),
        };

        let description: &str = take_until(1.., "\"").parse_next(input)?;
        let description = description.trim().to_string();

        Ok(Param {
            name,
            value,
            unit,
            description,
        })
    }

    let mut input = line;
    inner
        .parse_next(&mut input)
        .map_err(|_| anyhow::anyhow!("malformed old param line"))
}

fn parse_old_test(chunk: &str) -> Result<Test> {
    let chunk = chunk.trim_start();

    let (model, rest) = chunk
        .split_once(')')
        .ok_or_else(|| anyhow::anyhow!("missing closing ')' for model name"))?;
    let model = model.trim().to_string();

    let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();

    let (expression, rest) = rest.split_once("\n\tResult: ").ok_or_else(|| {
        anyhow::anyhow!(
            "missing 'Result:' line after expression {:?}",
            truncate(rest, 60)
        )
    })?;

    let expression = expression.trim().replace("par_", "").replace("**", "^");

    let (result_str, rest) = rest.split_once('\n').unwrap_or((rest, ""));
    let result_str = result_str.trim();

    let result = match result_str {
        "pass" => TestResult::Pass,
        "fail" => {
            let params = rest
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    parse_old_test_dependency_param(line)
                        .with_context(|| format!("dependency param #{}", i + 1))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();
            TestResult::Fail(params)
        }
        other => bail!(
            "expected test result 'pass' or 'fail', got {other:?} \
             (model: {model:?}, expression: {expression:?})"
        ),
    };

    Ok(Test {
        model,
        expression,
        result,
    })
}

fn parse_old_test_dependency_param(line: &str) -> Result<Option<TestDependencyParam>> {
    let Some((name, rest)) = line.split_once(':') else {
        return Ok(None);
    };
    let name = name.trim().to_string();

    let (value_str, unit) = split_value_unit(rest.trim(), " ");

    let value =
        parse_param_value(&value_str).with_context(|| format!("param {name:?} value parsing"))?;

    Ok(Some(TestDependencyParam {
        name,
        value,
        unit,
    }))
}

// ---------------------------------------------------------------------------
// New output parsing
// ---------------------------------------------------------------------------

const DIVIDER_LINE: &str =
    "────────────────────────────────────────────────────────────────────────────────\n";

/// Parse the new Oneil output format (from `oneil eval` + `oneil test`).
pub fn parse_new_output(output: &str) -> Result<(Vec<Param>, Vec<Test>)> {
    let params = output
        .lines()
        .filter(|line| line.contains('#'))
        .enumerate()
        .map(|(i, line)| {
            parse_new_param(line)
                .with_context(|| format!("new param #{} (line: {:?})", i + 1, truncate(line, 80)))
        })
        .collect::<Result<Vec<_>>>()
        .context("failed to parse new output parameters")?;

    let sections: Vec<&str> = output.split(DIVIDER_LINE).collect();
    let test_section = sections.get(2).ok_or_else(|| {
        anyhow::anyhow!(
            "expected at least 3 sections separated by divider lines, found {}",
            sections.len()
        )
    })?;

    let tests = test_section
        .split("\n\n")
        .enumerate()
        .map(|(i, group)| {
            parse_new_test_group(group).with_context(|| {
                format!(
                    "new test group #{} (starts with: {:?})",
                    i + 1,
                    truncate(group.trim(), 60)
                )
            })
        })
        .collect::<Result<Vec<_>>>()
        .context("failed to parse new output tests")?
        .into_iter()
        .flatten()
        .collect();

    Ok((params, tests))
}

fn parse_new_param(line: &str) -> Result<Param> {
    fn inner(input: &mut &str) -> ModalResult<Param> {
        let name: &str = take_until(1.., "=").parse_next(input)?;
        let name = name.trim().to_string();
        "=".parse_next(input)?;

        space0.parse_next(input)?;
        let value_and_unit: &str = take_until(1.., "#").parse_next(input)?;
        "#".parse_next(input)?;

        let (value_str, unit) = split_value_unit(value_and_unit.trim(), ":");
        let value = parse_param_value(&value_str)
            .map_err(|_| winnow::error::ErrMode::from_input(input))?;

        space0.parse_next(input)?;
        let description: &str = rest.parse_next(input)?;
        let description = description.trim().to_string();

        Ok(Param {
            name,
            value,
            unit,
            description,
        })
    }

    let mut input = line;
    inner
        .parse_next(&mut input)
        .map_err(|_| anyhow::anyhow!("malformed new param line"))
}

fn parse_new_test_group(group: &str) -> Result<Vec<Test>> {
    if group.trim().is_empty() {
        return Ok(vec![]);
    }

    let (model, rest) = group.split_once(".on\n").ok_or_else(|| {
        anyhow::anyhow!(
            "expected '.on' model file suffix in test group header: {:?}",
            truncate(group.trim(), 60)
        )
    })?;
    let model = model.trim().to_string();

    rest.split("test: ")
        .skip(1)
        .enumerate()
        .map(|(i, test_str)| {
            parse_new_test(&model, test_str)
                .with_context(|| format!("test #{} in model {model:?}", i + 1))
        })
        .collect()
}

fn parse_new_test(model: &str, test_str: &str) -> Result<Test> {
    let (expression, rest) = test_str.split_once("\n  Result: ").ok_or_else(|| {
        anyhow::anyhow!(
            "missing 'Result:' line in test: {:?}",
            truncate(test_str.trim(), 60)
        )
    })?;

    let expression = expression.trim().to_string();

    let (result_str, rest) = rest.split_once('\n').unwrap_or((rest, ""));
    let result_str = result_str.trim();

    let result = match result_str {
        "PASS" => TestResult::Pass,
        "FAIL" => {
            let params = rest
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    parse_new_test_dependency_param(line)
                        .with_context(|| format!("dependency param #{}", i + 1))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();
            TestResult::Fail(params)
        }
        other => bail!(
            "expected test result 'PASS' or 'FAIL', got {other:?} \
             (model: {model:?}, expression: {expression:?})"
        ),
    };

    Ok(Test {
        model: model.to_string(),
        expression,
        result,
    })
}

fn parse_new_test_dependency_param(line: &str) -> Result<Option<TestDependencyParam>> {
    if line.trim().is_empty() {
        return Ok(None);
    }

    let line = line.trim_start().strip_prefix("- ").unwrap_or(line);

    let (name, rest) = line
        .split_once(" = ")
        .ok_or_else(|| anyhow::anyhow!("missing ' = ' separator in: {:?}", truncate(line, 60)))?;

    let name = name.trim().to_string();
    let (value_str, unit) = split_value_unit(rest.trim(), " :");

    let value =
        parse_param_value(&value_str).with_context(|| format!("param {name:?} value parsing"))?;

    Ok(Some(TestDependencyParam {
        name,
        value,
        unit,
    }))
}
