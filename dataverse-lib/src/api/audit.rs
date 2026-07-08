//! Record audit history (`RetrieveRecordChangeHistory`).
//!
//! Retrieves the field-level change history for a single record via the
//! unbound `RetrieveRecordChangeHistory` function, returning per-attribute
//! old/new values.

use reqwest::Method;
use serde::Deserialize;
use uuid::Uuid;

use crate::DataverseClient;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;

/// A collection of audit details for a record, as returned by
/// `RetrieveRecordChangeHistory`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuditDetailCollection {
    /// One entry per audit event (attribute changes, relationship changes, etc.).
    pub audit_details: Vec<AuditDetail>,
    /// Whether more records are available beyond this page.
    pub more_records: bool,
    /// Opaque cursor for fetching the next page (empty when none).
    #[serde(default)]
    pub paging_cookie: String,
    /// Total record count, or `-1` when not computed.
    pub total_record_count: i64,
}

/// A single audit detail entry.
///
/// The API returns several concrete types (`AttributeAuditDetail`,
/// `RelationshipAuditDetail`, ...) distinguished by [`Self::odata_type`]. Only
/// attribute-change details carry [`Self::old_value`] / [`Self::new_value`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuditDetail {
    /// The `@odata.type` discriminator (e.g. `#Microsoft.Dynamics.CRM.AttributeAuditDetail`).
    ///
    /// Absent when the `AuditDetails` collection is homogeneous — OData omits the
    /// per-item type annotation in that case.
    #[serde(rename = "@odata.type", default)]
    pub odata_type: Option<String>,
    /// The underlying `audit` record (raw fields: `action`, `operation`,
    /// `createdon`, `_userid_value`, ...).
    pub audit_record: Record,
    /// Partial record of the changed fields' previous values (attribute details only).
    #[serde(default)]
    pub old_value: Option<Record>,
    /// Partial record of the changed fields' new values (attribute details only).
    #[serde(default)]
    pub new_value: Option<Record>,
}

impl AuditDetail {
    /// Returns `true` when this detail is an attribute-change detail
    /// (carries `old_value` / `new_value`).
    ///
    /// When the type annotation is absent (homogeneous collection), falls back to
    /// the presence of an old/new value record, which only attribute details have.
    pub fn is_attribute_change(&self) -> bool {
        match &self.odata_type {
            Some(t) => t.ends_with("AttributeAuditDetail"),
            None => self.old_value.is_some() || self.new_value.is_some(),
        }
    }
}

/// Response envelope for `RetrieveRecordChangeHistory`.
#[derive(Debug, Deserialize)]
struct RetrieveRecordChangeHistoryResponse {
    #[serde(rename = "AuditDetailCollection")]
    audit_detail_collection: AuditDetailCollection,
}

impl DataverseClient {
    /// Retrieves the change history (audit trail) for a single record.
    ///
    /// Calls the unbound `RetrieveRecordChangeHistory` function and returns the
    /// full [`AuditDetailCollection`], including non-attribute detail types.
    /// Callers that only want field changes can filter with
    /// [`AuditDetail::is_attribute_change`].
    pub async fn retrieve_record_change_history(
        &self,
        entity: &Entity,
        id: Uuid,
    ) -> Result<AuditDetailCollection, Error> {
        // Resolve to the entity set name for the Target reference.
        let logical_name = self.resolve_entity_logical_name(entity).await?;
        let set_name = self.resolve_entity_set_name(&logical_name).await?;

        // Build the Target parameter as a URL-encoded JSON entity reference.
        let target = format!(r#"{{"@odata.id":"{set_name}({id})"}}"#);
        let encoded = urlencoding::encode(&target);
        let path = format!("/RetrieveRecordChangeHistory(Target=@t)?@t={encoded}");
        let url = self.build_url(&path);

        let response = self.request(Method::GET, &url, None, None).await?;
        let body = response
            .text()
            .await
            .map_err(|e| Error::Api(ApiError::from(e)))?;

        let parsed: RetrieveRecordChangeHistoryResponse = serde_json::from_str(&body)?;
        Ok(parsed.audit_detail_collection)
    }
}
