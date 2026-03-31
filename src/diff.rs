use std::collections::HashMap;

use crate::parse::{Param, ParamValue, Test, TestDependencyParam, TestResult};

#[derive(Debug, Clone)]
pub enum Diff<T> {
    OldOnly(T),
    Same(T),
    Different(T, T),
    NewOnly(T),
}

impl<T> Diff<T> {
    pub fn first(&self) -> &T {
        match self {
            Diff::OldOnly(a) | Diff::Same(a) | Diff::Different(a, _) | Diff::NewOnly(a) => a,
        }
    }
}

pub fn diff_params(
    old_params: &[Param],
    new_params: &[Param],
    ignore_params: &[String],
) -> Vec<Diff<Param>> {
    let old_map: HashMap<&str, &Param> = old_params.iter().map(|p| (p.name.as_str(), p)).collect();
    let new_map: HashMap<&str, &Param> = new_params.iter().map(|p| (p.name.as_str(), p)).collect();

    let mut all_names: Vec<&str> = old_map
        .keys()
        .chain(new_map.keys())
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    all_names.sort();

    all_names
        .into_iter()
        .map(|name| {
            let raw_diff = match (old_map.get(name), new_map.get(name)) {
                (Some(old), None) => Diff::OldOnly((*old).clone()),
                (None, Some(new)) => Diff::NewOnly((*new).clone()),
                (Some(old), Some(new)) if params_equal(old, new) => Diff::Same((*old).clone()),
                (Some(old), Some(new)) => Diff::Different((*old).clone(), (*new).clone()),
                (None, None) => unreachable!(),
            };

            if ignore_params.iter().any(|p| p == name) {
                force_same(raw_diff)
            } else {
                raw_diff
            }
        })
        .collect()
}

pub fn diff_tests(
    old_tests: &[Test],
    new_tests: &[Test],
    ignore_tests: &[String],
) -> Vec<Diff<Test>> {
    let old_map: HashMap<&str, &Test> =
        old_tests.iter().map(|t| (t.expression.as_str(), t)).collect();
    let new_map: HashMap<&str, &Test> =
        new_tests.iter().map(|t| (t.expression.as_str(), t)).collect();

    let mut all_exprs: Vec<&str> = old_map
        .keys()
        .chain(new_map.keys())
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    all_exprs.sort();

    all_exprs
        .into_iter()
        .map(|expr| {
            let raw_diff = match (old_map.get(expr), new_map.get(expr)) {
                (Some(old), None) => Diff::OldOnly((*old).clone()),
                (None, Some(new)) => Diff::NewOnly((*new).clone()),
                (Some(old), Some(new)) if tests_equal(old, new) => Diff::Same((*old).clone()),
                (Some(old), Some(new)) => Diff::Different((*old).clone(), (*new).clone()),
                (None, None) => unreachable!(),
            };

            if ignore_tests.iter().any(|t| t == expr) {
                force_same(raw_diff)
            } else {
                raw_diff
            }
        })
        .collect()
}

fn force_same<T>(diff: Diff<T>) -> Diff<T> {
    match diff {
        Diff::OldOnly(a) | Diff::Same(a) => Diff::Same(a),
        Diff::Different(_, b) | Diff::NewOnly(b) => Diff::Same(b),
    }
}

fn params_equal(a: &Param, b: &Param) -> bool {
    a.name == b.name
        && value_is_close(&a.value, &b.value)
        && a.unit == b.unit
        && a.description == b.description
}

fn tests_equal(a: &Test, b: &Test) -> bool {
    a.model == b.model && a.expression == b.expression && test_results_equal(&a.result, &b.result)
}

fn test_results_equal(a: &TestResult, b: &TestResult) -> bool {
    match (a, b) {
        (TestResult::Pass, TestResult::Pass) => true,
        (TestResult::Fail(a_params), TestResult::Fail(b_params)) => {
            test_dependency_params_are_close(a_params, b_params)
        }
        _ => false,
    }
}

fn test_dependency_params_are_close(
    old_params: &[TestDependencyParam],
    new_params: &[TestDependencyParam],
) -> bool {
    if old_params.len() != new_params.len() {
        return false;
    }

    let old_map: HashMap<&str, &TestDependencyParam> =
        old_params.iter().map(|p| (p.name.as_str(), p)).collect();
    let new_map: HashMap<&str, &TestDependencyParam> =
        new_params.iter().map(|p| (p.name.as_str(), p)).collect();

    old_map.iter().all(|(name, old)| {
        new_map
            .get(name)
            .is_some_and(|new| old.unit == new.unit && value_is_close(&old.value, &new.value))
    })
}

fn value_is_close(a: &ParamValue, b: &ParamValue) -> bool {
    match (a, b) {
        (ParamValue::Scalar(a), ParamValue::Scalar(b)) => is_close(*a, *b),
        (
            ParamValue::Interval {
                min: a_min,
                max: a_max,
            },
            ParamValue::Interval {
                min: b_min,
                max: b_max,
            },
        ) => is_close(*a_min, *b_min) && is_close(*a_max, *b_max),
        (ParamValue::EmptyInterval, ParamValue::EmptyInterval) => true,
        (ParamValue::Str(a), ParamValue::Str(b)) => a == b,
        _ => false,
    }
}

fn is_close(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }

    let tolerance = 1e-3;
    let difference = (a - b).abs();
    let relative_tolerance = tolerance * a.abs().min(b.abs());

    difference <= relative_tolerance || difference <= tolerance
}
