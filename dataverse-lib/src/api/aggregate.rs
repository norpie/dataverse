//! Aggregation query builder.
//!
//! This module provides the `AggregateBuilder` for constructing aggregation queries
//! using FetchXML internally. FetchXML is the only query language in Dataverse
//! that supports aggregations like sum, count, avg, min, and max with group by.
//!
//! # Example
//!
//! ```ignore
//! let results = client.aggregate(Entity::logical("opportunity"))
//!     .group_by("ownerid", "owner")
//!     .sum("estimatedvalue", "total_value")
//!     .count("opportunityid", "count")
//!     .filter(Filter::eq("statecode", 0))
//!     .execute()
//!     .await?;
//!
//! for result in results {
//!     println!("Owner: {:?}, Total: {:?}, Count: {:?}",
//!         result.get("owner"),
//!         result.get("total_value"),
//!         result.get("count"));
//! }
//! ```

use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::Method;
use serde::Deserialize;
use url::form_urlencoded;

use crate::api::query::fetchxml::xml::escape_xml;
use crate::api::query::fetchxml::xml::filter_to_fetchxml;
use crate::api::query::Filter;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;
use crate::DataverseClient;

/// The type of aggregation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateType {
    /// Count of records.
    Count,
    /// Count of distinct values.
    CountDistinct,
    /// Sum of numeric values.
    Sum,
    /// Average of numeric values.
    Avg,
    /// Minimum value.
    Min,
    /// Maximum value.
    Max,
}

impl AggregateType {
    fn to_fetchxml(&self) -> &'static str {
        match self {
            AggregateType::Count => "count",
            AggregateType::CountDistinct => "countcolumn",
            AggregateType::Sum => "sum",
            AggregateType::Avg => "avg",
            AggregateType::Min => "min",
            AggregateType::Max => "max",
        }
    }
}

/// An aggregate column specification.
#[derive(Debug, Clone)]
struct AggregateColumn {
    /// The field to aggregate.
    field: String,
    /// The alias for the result.
    alias: String,
    /// The aggregation type.
    aggregate_type: AggregateType,
    /// Whether this is a distinct count.
    distinct: bool,
}

/// A group by column specification.
#[derive(Debug, Clone)]
struct GroupByColumn {
    /// The field to group by.
    field: String,
    /// The alias for the result.
    alias: String,
    /// Date grouping (for datetime fields).
    date_grouping: Option<DateGrouping>,
}

/// Date grouping options for datetime fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateGrouping {
    /// Group by day.
    Day,
    /// Group by week.
    Week,
    /// Group by month.
    Month,
    /// Group by quarter.
    Quarter,
    /// Group by year.
    Year,
    /// Group by fiscal period.
    FiscalPeriod,
    /// Group by fiscal year.
    FiscalYear,
}

impl DateGrouping {
    fn to_fetchxml(&self) -> &'static str {
        match self {
            DateGrouping::Day => "day",
            DateGrouping::Week => "week",
            DateGrouping::Month => "month",
            DateGrouping::Quarter => "quarter",
            DateGrouping::Year => "year",
            DateGrouping::FiscalPeriod => "fiscal-period",
            DateGrouping::FiscalYear => "fiscal-year",
        }
    }
}

/// Builder for constructing aggregation queries.
///
/// Uses FetchXML internally to perform aggregations.
///
/// # Example
///
/// ```ignore
/// let results = client.aggregate(Entity::logical("opportunity"))
///     .group_by("ownerid", "owner")
///     .sum("estimatedvalue", "total_value")
///     .count("opportunityid", "count")
///     .filter(Filter::eq("statecode", 0))
///     .execute()
///     .await?;
/// ```
pub struct AggregateBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    group_by: Vec<GroupByColumn>,
    aggregates: Vec<AggregateColumn>,
    filter: Option<Filter>,
    distinct: bool,
}

impl<'a> AggregateBuilder<'a> {
    /// Creates a new aggregate builder for the given entity.
    pub(crate) fn new(client: &'a DataverseClient, entity: Entity) -> Self {
        Self {
            client,
            entity,
            group_by: Vec::new(),
            aggregates: Vec::new(),
            filter: None,
            distinct: false,
        }
    }

    /// Adds a group by column.
    ///
    /// # Arguments
    ///
    /// * `field` - The field to group by
    /// * `alias` - The alias for this field in the results
    pub fn group_by(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.group_by.push(GroupByColumn {
            field: field.into(),
            alias: alias.into(),
            date_grouping: None,
        });
        self
    }

    /// Adds a group by column with date grouping.
    ///
    /// Use this for datetime fields to group by day, week, month, etc.
    ///
    /// # Arguments
    ///
    /// * `field` - The datetime field to group by
    /// * `alias` - The alias for this field in the results
    /// * `grouping` - The date grouping interval
    pub fn group_by_date(
        mut self,
        field: impl Into<String>,
        alias: impl Into<String>,
        grouping: DateGrouping,
    ) -> Self {
        self.group_by.push(GroupByColumn {
            field: field.into(),
            alias: alias.into(),
            date_grouping: Some(grouping),
        });
        self
    }

    /// Adds a count aggregation.
    ///
    /// # Arguments
    ///
    /// * `field` - The field to count
    /// * `alias` - The alias for the count in the results
    pub fn count(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.aggregates.push(AggregateColumn {
            field: field.into(),
            alias: alias.into(),
            aggregate_type: AggregateType::Count,
            distinct: false,
        });
        self
    }

    /// Adds a count distinct aggregation.
    ///
    /// # Arguments
    ///
    /// * `field` - The field to count distinct values
    /// * `alias` - The alias for the count in the results
    pub fn count_distinct(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.aggregates.push(AggregateColumn {
            field: field.into(),
            alias: alias.into(),
            aggregate_type: AggregateType::CountDistinct,
            distinct: true,
        });
        self
    }

    /// Adds a sum aggregation.
    ///
    /// # Arguments
    ///
    /// * `field` - The numeric field to sum
    /// * `alias` - The alias for the sum in the results
    pub fn sum(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.aggregates.push(AggregateColumn {
            field: field.into(),
            alias: alias.into(),
            aggregate_type: AggregateType::Sum,
            distinct: false,
        });
        self
    }

    /// Adds an average aggregation.
    ///
    /// # Arguments
    ///
    /// * `field` - The numeric field to average
    /// * `alias` - The alias for the average in the results
    pub fn avg(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.aggregates.push(AggregateColumn {
            field: field.into(),
            alias: alias.into(),
            aggregate_type: AggregateType::Avg,
            distinct: false,
        });
        self
    }

    /// Adds a minimum aggregation.
    ///
    /// # Arguments
    ///
    /// * `field` - The field to find the minimum value
    /// * `alias` - The alias for the minimum in the results
    pub fn min(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.aggregates.push(AggregateColumn {
            field: field.into(),
            alias: alias.into(),
            aggregate_type: AggregateType::Min,
            distinct: false,
        });
        self
    }

    /// Adds a maximum aggregation.
    ///
    /// # Arguments
    ///
    /// * `field` - The field to find the maximum value
    /// * `alias` - The alias for the maximum in the results
    pub fn max(mut self, field: impl Into<String>, alias: impl Into<String>) -> Self {
        self.aggregates.push(AggregateColumn {
            field: field.into(),
            alias: alias.into(),
            aggregate_type: AggregateType::Max,
            distinct: false,
        });
        self
    }

    /// Adds a filter condition.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Sets the query to return distinct results.
    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    /// Returns the entity logical name.
    fn entity_logical_name(&self) -> &str {
        match &self.entity {
            Entity::Set(name) => name,
            Entity::Logical(name) => name,
        }
    }

    /// Builds the FetchXML for this aggregation query.
    pub fn to_fetchxml(&self) -> String {
        let entity_name = self.entity_logical_name();

        // Build fetch attributes
        let mut fetch_attrs = vec![
            r#"version="1.0""#.to_string(),
            r#"aggregate="true""#.to_string(),
            r#"mapping="logical""#.to_string(),
        ];

        if self.distinct {
            fetch_attrs.push(r#"distinct="true""#.to_string());
        }

        // Build entity content
        let mut entity_content = String::new();

        // Group by columns
        for group in &self.group_by {
            let date_grouping_attr = group
                .date_grouping
                .as_ref()
                .map(|dg| format!(r#" dategrouping="{}""#, dg.to_fetchxml()))
                .unwrap_or_default();

            entity_content.push_str(&format!(
                r#"<attribute name="{}" alias="{}" groupby="true"{}/>
"#,
                escape_xml(&group.field),
                escape_xml(&group.alias),
                date_grouping_attr
            ));
        }

        // Aggregate columns
        for agg in &self.aggregates {
            let distinct_attr = if agg.distinct {
                r#" distinct="true""#
            } else {
                ""
            };

            entity_content.push_str(&format!(
                r#"<attribute name="{}" alias="{}" aggregate="{}"{}/>
"#,
                escape_xml(&agg.field),
                escape_xml(&agg.alias),
                agg.aggregate_type.to_fetchxml(),
                distinct_attr
            ));
        }

        // Filter
        if let Some(ref filter) = self.filter {
            let filter_xml = filter_to_fetchxml(filter);
            // Wrap in filter element if it's just conditions
            if !filter_xml.starts_with("<filter") {
                entity_content.push_str(&format!(r#"<filter type="and">{}</filter>"#, filter_xml));
            } else {
                entity_content.push_str(&filter_xml);
            }
        }

        format!(
            r#"<fetch {}><entity name="{}">{}</entity></fetch>"#,
            fetch_attrs.join(" "),
            escape_xml(entity_name),
            entity_content
        )
    }

    /// Executes the aggregation query and returns the results.
    pub async fn execute(self) -> Result<Vec<Record>, Error> {
        // Resolve entity set name
        let entity_set_name = match &self.entity {
            Entity::Set(name) => name.clone(),
            Entity::Logical(logical_name) => {
                self.client.resolve_entity_set_name(logical_name).await?
            }
        };

        // Build FetchXML
        let fetchxml = self.to_fetchxml();

        // Build URL
        let base_url = self.client.base_url().trim_end_matches('/');
        let api_version = self.client.api_version();
        let encoded_fetchxml: String =
            form_urlencoded::byte_serialize(fetchxml.as_bytes()).collect();
        let url = format!(
            "{}/api/data/{}/{}?fetchXml={}",
            base_url, api_version, entity_set_name, encoded_fetchxml
        );

        // Build headers
        let mut headers = HeaderMap::new();
        headers.insert("OData-MaxVersion", HeaderValue::from_static("4.0"));
        headers.insert("OData-Version", HeaderValue::from_static("4.0"));
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(
            "Prefer",
            HeaderValue::from_static("odata.include-annotations=\"*\""),
        );

        // Make request
        let response: reqwest::Response = self
            .client
            .request(Method::GET, &url, headers, None)
            .await?;

        // Parse response
        let aggregate_response: AggregateResponse = response.json().await.map_err(ApiError::from)?;

        Ok(aggregate_response.value)
    }
}

/// Response structure for aggregate queries.
#[derive(Debug, Deserialize)]
struct AggregateResponse {
    /// The aggregate results.
    value: Vec<Record>,
}
