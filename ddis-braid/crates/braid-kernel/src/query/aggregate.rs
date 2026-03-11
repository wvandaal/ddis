//! Post-processing aggregation for Datalog query results.
//!
//! Applies aggregate functions (COUNT, SUM, MIN, MAX, AVG) to query result
//! rows AFTER Datalog evaluation. This keeps the core evaluator pure and
//! simple while enabling analytical queries.
//!
//! Aggregation operates only on `QueryResult::Rel` results. Scalar results
//! are returned unchanged since they already contain a single value.
//!
//! Traces to: INV-QUERY-001 (query correctness), ADR-QUERY-001 (Datalog foundation).

use ordered_float::OrderedFloat;

use crate::datom::Value;
use crate::query::QueryResult;

/// Supported aggregate functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateFunction {
    /// Count of rows in the group.
    Count,
    /// Sum of numeric values.
    Sum,
    /// Minimum value (numeric or lexicographic comparison).
    Min,
    /// Maximum value (numeric or lexicographic comparison).
    Max,
    /// Average of numeric values.
    Avg,
}

impl AggregateFunction {
    /// Parse an aggregate function name (case-insensitive).
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "count" => Some(Self::Count),
            "sum" => Some(Self::Sum),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "avg" => Some(Self::Avg),
            _ => None,
        }
    }
}

/// An aggregate specification: function + target column index.
#[derive(Clone, Debug)]
pub struct AggregateSpec {
    /// The aggregate function to apply.
    pub function: AggregateFunction,
    /// Column index to aggregate over.
    pub column: usize,
    /// Output column name.
    pub output_name: String,
}

/// Apply aggregation to a `QueryResult`.
///
/// Only `QueryResult::Rel` results are aggregated. Scalar results pass through
/// unchanged since they already contain a single value.
///
/// When `group_by` is empty, aggregates over all rows (single output row).
/// When `group_by` contains column indices, groups rows by those columns
/// and applies aggregates per group.
pub fn aggregate(
    result: &QueryResult,
    aggregates: &[AggregateSpec],
    group_by: &[usize],
) -> QueryResult {
    let rows = match result {
        QueryResult::Rel(rows) => rows,
        QueryResult::Scalar(_) => return result.clone(),
    };

    if rows.is_empty() || aggregates.is_empty() {
        return result.clone();
    }

    if group_by.is_empty() {
        // Global aggregation — one output row
        let row: Vec<Value> = aggregates
            .iter()
            .map(|spec| compute_aggregate(&spec.function, rows, spec.column))
            .collect();
        QueryResult::Rel(vec![row])
    } else {
        // Grouped aggregation
        let mut groups: std::collections::BTreeMap<Vec<String>, Vec<&Vec<Value>>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let key: Vec<String> = group_by
                .iter()
                .map(|&i| {
                    if i < row.len() {
                        format_value_for_grouping(&row[i])
                    } else {
                        String::new()
                    }
                })
                .collect();
            groups.entry(key).or_default().push(row);
        }

        let mut out_rows = Vec::with_capacity(groups.len());
        for (key, group_rows) in &groups {
            let mut row: Vec<Value> = key.iter().map(|k| Value::String(k.clone())).collect();
            for spec in aggregates {
                row.push(compute_aggregate_from_refs(
                    &spec.function,
                    group_rows,
                    spec.column,
                ));
            }
            out_rows.push(row);
        }

        QueryResult::Rel(out_rows)
    }
}

/// Compute an aggregate over all rows for a given column.
fn compute_aggregate(func: &AggregateFunction, rows: &[Vec<Value>], col: usize) -> Value {
    compute_aggregate_from_refs(func, &rows.iter().collect::<Vec<_>>(), col)
}

/// Compute an aggregate over referenced rows for a given column.
fn compute_aggregate_from_refs(
    func: &AggregateFunction,
    rows: &[&Vec<Value>],
    col: usize,
) -> Value {
    match func {
        AggregateFunction::Count => Value::Long(rows.len() as i64),
        AggregateFunction::Sum => {
            let sum: f64 = rows
                .iter()
                .filter_map(|row| row.get(col).and_then(value_to_f64))
                .sum();
            Value::Double(OrderedFloat(sum))
        }
        AggregateFunction::Avg => {
            let values: Vec<f64> = rows
                .iter()
                .filter_map(|row| row.get(col).and_then(value_to_f64))
                .collect();
            if values.is_empty() {
                Value::Double(OrderedFloat(0.0))
            } else {
                Value::Double(OrderedFloat(
                    values.iter().sum::<f64>() / values.len() as f64,
                ))
            }
        }
        AggregateFunction::Min => rows
            .iter()
            .filter_map(|row| row.get(col))
            .min_by(|a, b| compare_values(a, b))
            .cloned()
            .unwrap_or(Value::String("(empty)".into())),
        AggregateFunction::Max => rows
            .iter()
            .filter_map(|row| row.get(col))
            .max_by(|a, b| compare_values(a, b))
            .cloned()
            .unwrap_or(Value::String("(empty)".into())),
    }
}

/// Extract a numeric value from a `Value` for arithmetic aggregation.
fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Long(n) => Some(*n as f64),
        Value::Double(d) => Some(d.into_inner()),
        Value::Instant(t) => Some(*t as f64),
        _ => None,
    }
}

/// Compare two `Value`s for ordering (used by Min/Max).
fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Long(x), Value::Long(y)) => x.cmp(y),
        (Value::Double(x), Value::Double(y)) => x.cmp(y),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Keyword(x), Value::Keyword(y)) => x.cmp(y),
        (Value::Instant(x), Value::Instant(y)) => x.cmp(y),
        // Cross-type: numeric coercion, then fallback to debug repr
        (Value::Long(x), Value::Double(y)) => OrderedFloat(*x as f64).cmp(y),
        (Value::Double(x), Value::Long(y)) => x.cmp(&OrderedFloat(*y as f64)),
        _ => format!("{a:?}").cmp(&format!("{b:?}")),
    }
}

/// Format a `Value` as a string for grouping keys.
fn format_value_for_grouping(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Keyword(k) => k.clone(),
        Value::Long(n) => n.to_string(),
        Value::Double(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Ref(e) => format!("{e:?}"),
        Value::Instant(t) => t.to_string(),
        Value::Uuid(u) => format!("{u:?}"),
        Value::Bytes(b) => format!("{b:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(rows: Vec<Vec<Value>>) -> QueryResult {
        QueryResult::Rel(rows)
    }

    #[test]
    fn count_all_rows() {
        let result = rel(vec![
            vec![Value::String("a".into())],
            vec![Value::String("b".into())],
            vec![Value::String("c".into())],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Count,
                column: 0,
                output_name: "count".into(),
            }],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0][0], Value::Long(3));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn sum_numeric_values() {
        let result = rel(vec![
            vec![Value::Long(10)],
            vec![Value::Long(20)],
            vec![Value::Long(30)],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Sum,
                column: 0,
                output_name: "total".into(),
            }],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows[0][0], Value::Double(OrderedFloat(60.0)));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn avg_numeric_values() {
        let result = rel(vec![
            vec![Value::Long(10)],
            vec![Value::Long(20)],
            vec![Value::Long(30)],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Avg,
                column: 0,
                output_name: "average".into(),
            }],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows[0][0], Value::Double(OrderedFloat(20.0)));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn min_max_values() {
        let result = rel(vec![
            vec![Value::Long(30)],
            vec![Value::Long(10)],
            vec![Value::Long(20)],
        ]);
        let min_agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Min,
                column: 0,
                output_name: "min".into(),
            }],
            &[],
        );
        match &min_agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows[0][0], Value::Long(10));
            }
            _ => panic!("expected Rel"),
        }

        let max_agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Max,
                column: 0,
                output_name: "max".into(),
            }],
            &[],
        );
        match &max_agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows[0][0], Value::Long(30));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn grouped_count() {
        let result = rel(vec![
            vec![Value::String("a".into()), Value::Long(1)],
            vec![Value::String("a".into()), Value::Long(2)],
            vec![Value::String("b".into()), Value::Long(3)],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Count,
                column: 1,
                output_name: "count".into(),
            }],
            &[0],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows.len(), 2);
                // BTreeMap ordering: "a" before "b"
                let a_row = rows
                    .iter()
                    .find(|r| r[0] == Value::String("a".into()))
                    .unwrap();
                assert_eq!(a_row[1], Value::Long(2));
                let b_row = rows
                    .iter()
                    .find(|r| r[0] == Value::String("b".into()))
                    .unwrap();
                assert_eq!(b_row[1], Value::Long(1));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn grouped_sum() {
        let result = rel(vec![
            vec![Value::String("x".into()), Value::Long(10)],
            vec![Value::String("x".into()), Value::Long(20)],
            vec![Value::String("y".into()), Value::Long(5)],
            vec![Value::String("y".into()), Value::Long(15)],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Sum,
                column: 1,
                output_name: "total".into(),
            }],
            &[0],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows.len(), 2);
                let x_row = rows
                    .iter()
                    .find(|r| r[0] == Value::String("x".into()))
                    .unwrap();
                assert_eq!(x_row[1], Value::Double(OrderedFloat(30.0)));
                let y_row = rows
                    .iter()
                    .find(|r| r[0] == Value::String("y".into()))
                    .unwrap();
                assert_eq!(y_row[1], Value::Double(OrderedFloat(20.0)));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn empty_result_passthrough() {
        let result = rel(vec![]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Count,
                column: 0,
                output_name: "count".into(),
            }],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => assert!(rows.is_empty()),
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn scalar_passthrough() {
        let result = QueryResult::Scalar(Some(Value::Long(42)));
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Count,
                column: 0,
                output_name: "count".into(),
            }],
            &[],
        );
        match agg {
            QueryResult::Scalar(Some(Value::Long(42))) => {}
            other => panic!("expected Scalar(42), got {other:?}"),
        }
    }

    #[test]
    fn parse_aggregate_function() {
        assert_eq!(
            AggregateFunction::parse("COUNT"),
            Some(AggregateFunction::Count)
        );
        assert_eq!(
            AggregateFunction::parse("sum"),
            Some(AggregateFunction::Sum)
        );
        assert_eq!(
            AggregateFunction::parse("Min"),
            Some(AggregateFunction::Min)
        );
        assert_eq!(
            AggregateFunction::parse("MAX"),
            Some(AggregateFunction::Max)
        );
        assert_eq!(
            AggregateFunction::parse("avg"),
            Some(AggregateFunction::Avg)
        );
        assert_eq!(AggregateFunction::parse("unknown"), None);
    }

    #[test]
    fn min_max_strings() {
        let result = rel(vec![
            vec![Value::String("banana".into())],
            vec![Value::String("apple".into())],
            vec![Value::String("cherry".into())],
        ]);
        let min_agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Min,
                column: 0,
                output_name: "min".into(),
            }],
            &[],
        );
        match &min_agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows[0][0], Value::String("apple".into()));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn sum_with_doubles() {
        let result = rel(vec![
            vec![Value::Double(OrderedFloat(1.5))],
            vec![Value::Double(OrderedFloat(2.5))],
            vec![Value::Double(OrderedFloat(3.0))],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Sum,
                column: 0,
                output_name: "total".into(),
            }],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows[0][0], Value::Double(OrderedFloat(7.0)));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn multiple_aggregates_at_once() {
        let result = rel(vec![
            vec![Value::Long(10)],
            vec![Value::Long(20)],
            vec![Value::Long(30)],
        ]);
        let agg = aggregate(
            &result,
            &[
                AggregateSpec {
                    function: AggregateFunction::Count,
                    column: 0,
                    output_name: "count".into(),
                },
                AggregateSpec {
                    function: AggregateFunction::Sum,
                    column: 0,
                    output_name: "sum".into(),
                },
                AggregateSpec {
                    function: AggregateFunction::Avg,
                    column: 0,
                    output_name: "avg".into(),
                },
            ],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0][0], Value::Long(3));
                assert_eq!(rows[0][1], Value::Double(OrderedFloat(60.0)));
                assert_eq!(rows[0][2], Value::Double(OrderedFloat(20.0)));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn avg_skips_non_numeric() {
        let result = rel(vec![
            vec![Value::Long(10)],
            vec![Value::String("not a number".into())],
            vec![Value::Long(30)],
        ]);
        let agg = aggregate(
            &result,
            &[AggregateSpec {
                function: AggregateFunction::Avg,
                column: 0,
                output_name: "avg".into(),
            }],
            &[],
        );
        match &agg {
            QueryResult::Rel(rows) => {
                // Average of 10 and 30 (string skipped) = 20.0
                assert_eq!(rows[0][0], Value::Double(OrderedFloat(20.0)));
            }
            _ => panic!("expected Rel"),
        }
    }

    #[test]
    fn no_aggregates_passthrough() {
        let result = rel(vec![vec![Value::Long(1)], vec![Value::Long(2)]]);
        let agg = aggregate(&result, &[], &[]);
        match &agg {
            QueryResult::Rel(rows) => {
                assert_eq!(rows.len(), 2);
            }
            _ => panic!("expected Rel"),
        }
    }
}
