//! FetchXML generation utilities.

use crate::api::query::Direction;
use crate::api::query::Filter;
use crate::api::query::OrderBy;
use crate::model::Value;

/// Escapes a string for use in XML attribute values.
pub fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Converts a `Filter` to FetchXML `<filter>` and `<condition>` elements.
pub fn filter_to_fetchxml(filter: &Filter) -> String {
    match filter {
        Filter::Eq(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="eq" value="{}"/>"#,
                escape_xml(field),
                escape_xml(&value_to_fetchxml(value))
            )
        }
        Filter::Ne(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="ne" value="{}"/>"#,
                escape_xml(field),
                escape_xml(&value_to_fetchxml(value))
            )
        }
        Filter::Gt(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="gt" value="{}"/>"#,
                escape_xml(field),
                escape_xml(&value_to_fetchxml(value))
            )
        }
        Filter::Ge(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="ge" value="{}"/>"#,
                escape_xml(field),
                escape_xml(&value_to_fetchxml(value))
            )
        }
        Filter::Lt(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="lt" value="{}"/>"#,
                escape_xml(field),
                escape_xml(&value_to_fetchxml(value))
            )
        }
        Filter::Le(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="le" value="{}"/>"#,
                escape_xml(field),
                escape_xml(&value_to_fetchxml(value))
            )
        }
        Filter::Contains(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="like" value="%{}%"/>"#,
                escape_xml(field),
                escape_xml(value)
            )
        }
        Filter::StartsWith(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="like" value="{}%"/>"#,
                escape_xml(field),
                escape_xml(value)
            )
        }
        Filter::EndsWith(field, value) => {
            format!(
                r#"<condition attribute="{}" operator="like" value="%{}"/>"#,
                escape_xml(field),
                escape_xml(value)
            )
        }
        Filter::IsNull(field) => {
            format!(
                r#"<condition attribute="{}" operator="null"/>"#,
                escape_xml(field)
            )
        }
        Filter::IsNotNull(field) => {
            format!(
                r#"<condition attribute="{}" operator="not-null"/>"#,
                escape_xml(field)
            )
        }
        Filter::And(filters) => {
            if filters.is_empty() {
                return String::new();
            }
            let conditions: Vec<_> = filters.iter().map(filter_to_fetchxml).collect();
            format!(r#"<filter type="and">{}</filter>"#, conditions.join(""))
        }
        Filter::Or(filters) => {
            if filters.is_empty() {
                return String::new();
            }
            let conditions: Vec<_> = filters.iter().map(filter_to_fetchxml).collect();
            format!(r#"<filter type="or">{}</filter>"#, conditions.join(""))
        }
        Filter::Not(inner) => {
            // FetchXML doesn't have a direct NOT operator, we wrap in a filter
            // with negated conditions where possible
            format!(
                r#"<filter type="and"><condition operator="not">{}</condition></filter>"#,
                filter_to_fetchxml(inner)
            )
        }
        Filter::Raw(raw) => raw.clone(),
    }
}

/// Converts a `Value` to a FetchXML string representation.
pub fn value_to_fetchxml(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
        Value::Int(n) => n.to_string(),
        Value::Long(n) => n.to_string(),
        Value::Float(n) => n.to_string(),
        Value::Decimal(d) => d.to_string(),
        Value::String(s) => s.clone(),
        Value::Guid(g) => g.to_string(),
        Value::DateTime(dt) => dt.to_rfc3339(),
        Value::Money(m) => m.value().to_string(),
        Value::OptionSet(o) => o.value.to_string(),
        Value::EntityReference(r) => r.id.to_string(),
        Value::EntityBinding(b) => b.id.to_string(),
        // Complex types aren't typically used in filters
        Value::MultiOptionSet(_)
        | Value::File(_)
        | Value::Image(_)
        | Value::Record(_)
        | Value::Records(_)
        | Value::Json(_) => String::new(),
    }
}

/// Converts an `OrderBy` to FetchXML `<order>` elements.
pub fn order_to_fetchxml(order: &OrderBy) -> String {
    order
        .fields()
        .iter()
        .map(|(field, direction)| {
            let descending = match direction {
                Direction::Asc => "false",
                Direction::Desc => "true",
            };
            format!(
                r#"<order attribute="{}" descending="{}"/>"#,
                escape_xml(field),
                descending
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Generates `<attribute>` elements for a list of field names.
pub fn attributes_to_fetchxml(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| format!(r#"<attribute name="{}"/>"#, escape_xml(f)))
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("O'Brien & Co"), "O&apos;Brien &amp; Co");
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
    }

    #[test]
    fn test_simple_conditions() {
        assert_eq!(
            filter_to_fetchxml(&Filter::eq("name", "Contoso")),
            r#"<condition attribute="name" operator="eq" value="Contoso"/>"#
        );
        assert_eq!(
            filter_to_fetchxml(&Filter::gt("revenue", 1000000i32)),
            r#"<condition attribute="revenue" operator="gt" value="1000000"/>"#
        );
        assert_eq!(
            filter_to_fetchxml(&Filter::is_null("parentaccountid")),
            r#"<condition attribute="parentaccountid" operator="null"/>"#
        );
    }

    #[test]
    fn test_like_conditions() {
        assert_eq!(
            filter_to_fetchxml(&Filter::contains("name", "Corp")),
            r#"<condition attribute="name" operator="like" value="%Corp%"/>"#
        );
        assert_eq!(
            filter_to_fetchxml(&Filter::starts_with("name", "A")),
            r#"<condition attribute="name" operator="like" value="A%"/>"#
        );
    }

    #[test]
    fn test_combined_filters() {
        let filter = Filter::and([
            Filter::eq("statecode", 0i32),
            Filter::gt("revenue", 1000000i32),
        ]);
        assert_eq!(
            filter_to_fetchxml(&filter),
            r#"<filter type="and"><condition attribute="statecode" operator="eq" value="0"/><condition attribute="revenue" operator="gt" value="1000000"/></filter>"#
        );
    }

    #[test]
    fn test_order_by() {
        let order = OrderBy::desc("revenue").then_asc("name");
        assert_eq!(
            order_to_fetchxml(&order),
            r#"<order attribute="revenue" descending="true"/><order attribute="name" descending="false"/>"#
        );
    }

    #[test]
    fn test_attributes() {
        let fields = vec!["name".to_string(), "revenue".to_string()];
        assert_eq!(
            attributes_to_fetchxml(&fields),
            r#"<attribute name="name"/><attribute name="revenue"/>"#
        );
    }
}
