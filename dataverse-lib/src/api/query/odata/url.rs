//! OData URL and query string generation.

use crate::api::query::Direction;
use crate::api::query::Filter;
use crate::api::query::ODataFilter;
use crate::api::query::OrderBy;
use crate::model::Value;

/// Converts an `ODataFilter` to an OData `$filter` expression.
///
/// This handles both regular filters and negated filters.
pub fn odata_filter_to_string(filter: &ODataFilter) -> String {
    match filter {
        ODataFilter::Base(f) => filter_to_odata(f),
        ODataFilter::Not(inner) => format!("not ({})", odata_filter_to_string(inner)),
    }
}

/// Converts a `Filter` to an OData `$filter` expression.
pub fn filter_to_odata(filter: &Filter) -> String {
    match filter {
        Filter::Eq(field, value) => format!("{} eq {}", field, value_to_odata(value)),
        Filter::Ne(field, value) => format!("{} ne {}", field, value_to_odata(value)),
        Filter::Gt(field, value) => format!("{} gt {}", field, value_to_odata(value)),
        Filter::Ge(field, value) => format!("{} ge {}", field, value_to_odata(value)),
        Filter::Lt(field, value) => format!("{} lt {}", field, value_to_odata(value)),
        Filter::Le(field, value) => format!("{} le {}", field, value_to_odata(value)),
        Filter::Contains(field, value) => {
            format!("contains({},{})", field, escape_string(value))
        }
        Filter::StartsWith(field, value) => {
            format!("startswith({},{})", field, escape_string(value))
        }
        Filter::EndsWith(field, value) => {
            format!("endswith({},{})", field, escape_string(value))
        }
        Filter::IsNull(field) => format!("{} eq null", field),
        Filter::IsNotNull(field) => format!("{} ne null", field),
        Filter::And(filters) => {
            if filters.is_empty() {
                return String::new();
            }
            let parts: Vec<_> = filters.iter().map(filter_to_odata).collect();
            format!("({})", parts.join(" and "))
        }
        Filter::Or(filters) => {
            if filters.is_empty() {
                return String::new();
            }
            let parts: Vec<_> = filters.iter().map(filter_to_odata).collect();
            format!("({})", parts.join(" or "))
        }
        Filter::Raw(raw) => raw.clone(),
    }
}

/// Converts a `Value` to an OData literal representation.
pub fn value_to_odata(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Float(n) => {
            // Ensure float has decimal point for OData
            let s = n.to_string();
            if s.contains('.') || s.contains('e') || s.contains('E') {
                s
            } else {
                format!("{}.0", s)
            }
        }
        Value::Decimal(d) => d.to_string(),
        Value::String(s) => escape_string(s),
        Value::Guid(g) => g.to_string(),
        Value::DateTime(dt) => dt.to_rfc3339(),
        Value::Money(m) => m.value().to_string(),
        Value::OptionSet(o) => o.value.to_string(),
        Value::EntityReference(r) => r.id.to_string(),
        Value::EntityBinding(b) => b.id.to_string(),
        // For complex types, fall back to JSON representation
        Value::MultiOptionSet(_)
        | Value::File(_)
        | Value::Image(_)
        | Value::Record(_)
        | Value::Records(_)
        | Value::Json(_) => {
            // These shouldn't typically be used in filters
            "null".to_string()
        }
    }
}

/// Converts an `OrderBy` to an OData `$orderby` expression.
pub fn order_to_odata(order: &OrderBy) -> String {
    order
        .fields()
        .iter()
        .map(|(field, direction)| {
            let dir = match direction {
                Direction::Asc => "asc",
                Direction::Desc => "desc",
            };
            format!("{} {}", field, dir)
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Escapes a string for use in OData queries.
///
/// OData strings are enclosed in single quotes, with internal single quotes doubled.
pub fn escape_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_filters() {
        assert_eq!(
            filter_to_odata(&Filter::eq("name", "Contoso")),
            "name eq 'Contoso'"
        );
        assert_eq!(
            filter_to_odata(&Filter::gt("revenue", 1000000i32)),
            "revenue gt 1000000"
        );
        assert_eq!(
            filter_to_odata(&Filter::is_null("parentaccountid")),
            "parentaccountid eq null"
        );
    }

    #[test]
    fn test_string_functions() {
        assert_eq!(
            filter_to_odata(&Filter::contains("name", "Corp")),
            "contains(name,'Corp')"
        );
        assert_eq!(
            filter_to_odata(&Filter::starts_with("name", "A")),
            "startswith(name,'A')"
        );
    }

    #[test]
    fn test_combined_filters() {
        let filter = Filter::and([
            Filter::eq("statecode", 0i32),
            Filter::gt("revenue", 1000000i32),
        ]);
        assert_eq!(
            filter_to_odata(&filter),
            "(statecode eq 0 and revenue gt 1000000)"
        );
    }

    #[test]
    fn test_order_by() {
        let order = OrderBy::desc("revenue").then_asc("name");
        assert_eq!(order_to_odata(&order), "revenue desc,name asc");
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("O'Brien"), "'O''Brien'");
    }

    #[test]
    fn test_negated_filter() {
        let filter = Filter::eq("statecode", 0i32).not();
        assert_eq!(odata_filter_to_string(&filter), "not (statecode eq 0)");
    }

    #[test]
    fn test_double_negated_filter() {
        let filter = Filter::eq("statecode", 0i32).not().not();
        assert_eq!(
            odata_filter_to_string(&filter),
            "not (not (statecode eq 0))"
        );
    }

    #[test]
    fn test_negated_combined_filter() {
        let filter = Filter::and([
            Filter::eq("statecode", 0i32),
            Filter::gt("revenue", 1000000i32),
        ])
        .not();
        assert_eq!(
            odata_filter_to_string(&filter),
            "not ((statecode eq 0 and revenue gt 1000000))"
        );
    }
}
