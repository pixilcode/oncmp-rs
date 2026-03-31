use anyhow::{Result, bail};
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
                _ => bail!("invalid interval: {input}"),
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
        .map(parse_old_param)
        .collect::<Result<Vec<_>>>()?;

    let tests = output
        .split("\nTest (")
        .skip(1)
        .map(parse_old_test)
        .collect::<Result<Vec<_>>>()?;

    Ok((params, tests))
}

fn parse_old_param(line: &str) -> Result<Param> {
    // Format: `name: value [unit] -- "description"`
    fn inner(input: &mut &str) -> ModalResult<Param> {
        let name = take_until(1.., ":").parse_next(input)?;
        let name = name.trim().to_string();
        ":".parse_next(input)?;

        space0.parse_next(input)?;
        let value_str: &str = take_until(1.., " ").parse_next(input)?;
        " ".parse_next(input)?;

        // Check if value is a repeated string like `my_str | my_str`
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
        .map_err(|e| anyhow::anyhow!("failed to parse old param: {e} (line: {line:?})"))
}

fn parse_old_test(chunk: &str) -> Result<Test> {
    let chunk = chunk.trim_start();

    let (model, rest) = chunk
        .split_once(')')
        .ok_or_else(|| anyhow::anyhow!("expected ')' in old test: {chunk:?}"))?;
    let model = model.trim().to_string();

    // Drop the colon after ')'
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim_start();

    let (expression, rest) = rest
        .split_once("\n\tResult: ")
        .ok_or_else(|| anyhow::anyhow!("expected '\\n\\tResult: ' in old test: {chunk:?}"))?;

    let expression = expression
        .trim()
        .replace("par_", "")
        .replace("**", "^");

    let (result_str, rest) = rest.split_once('\n').unwrap_or((rest, ""));
    let result_str = result_str.trim();

    let result = match result_str {
        "pass" => TestResult::Pass,
        "fail" => {
            let params = rest
                .lines()
                .map(parse_old_test_dependency_param)
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();
            TestResult::Fail(params)
        }
        _ => bail!("invalid old test result: {result_str:?} (chunk: {chunk:?})"),
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

    let value = parse_param_value(&value_str)?;

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
        .map(parse_new_param)
        .collect::<Result<Vec<_>>>()?;

    let sections: Vec<&str> = output.split(DIVIDER_LINE).collect();
    let test_section = sections
        .get(2)
        .ok_or_else(|| anyhow::anyhow!("no tests found in output"))?;

    let tests = test_section
        .split("\n\n")
        .map(parse_new_test_group)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok((params, tests))
}

fn parse_new_param(line: &str) -> Result<Param> {
    // Format: `name = value [:unit] # description`
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
        .map_err(|e| anyhow::anyhow!("failed to parse new param: {e} (line: {line:?})"))
}

fn parse_new_test_group(group: &str) -> Result<Vec<Test>> {
    if group.trim().is_empty() {
        return Ok(vec![]);
    }

    let (model, rest) = group
        .split_once(".on\n")
        .ok_or_else(|| anyhow::anyhow!("error parsing test group: {group}"))?;
    let model = model.trim().to_string();

    rest.split("test: ")
        .skip(1)
        .map(|test_str| parse_new_test(&model, test_str))
        .collect()
}

fn parse_new_test(model: &str, test_str: &str) -> Result<Test> {
    let (expression, rest) = test_str
        .split_once("\n  Result: ")
        .ok_or_else(|| anyhow::anyhow!("expected '\\n  Result: ' in new test: {test_str:?}"))?;

    let expression = expression.trim().to_string();

    let (result_str, rest) = rest.split_once('\n').unwrap_or((rest, ""));
    let result_str = result_str.trim();

    let result = match result_str {
        "PASS" => TestResult::Pass,
        "FAIL" => {
            let params = rest
                .lines()
                .map(parse_new_test_dependency_param)
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect();
            TestResult::Fail(params)
        }
        _ => bail!("invalid new test result: {result_str:?} (test: {test_str:?})"),
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

    // Drop the leading `  - ` prefix
    let line = line.trim_start().strip_prefix("- ").unwrap_or(line);

    let (name, rest) = line
        .split_once(" = ")
        .ok_or_else(|| anyhow::anyhow!("expected ' = ' in test dep param: {line:?}"))?;

    let name = name.trim().to_string();
    let (value_str, unit) = split_value_unit(rest.trim(), " :");

    let value = parse_param_value(&value_str)?;

    Ok(Some(TestDependencyParam {
        name,
        value,
        unit,
    }))
}
