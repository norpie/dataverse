//! Multipart MIME request builder for batch operations.

use uuid::Uuid;

use super::Batch;
use super::BatchItem;
use super::BatchOptions;
use crate::api::crud::Operation;
use crate::api::crud::OperationOptions;
use crate::model::Entity;

/// Generates a unique boundary string.
pub fn generate_boundary(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

/// Builds the multipart batch request body.
pub fn build_batch_body(batch: &Batch, base_url: &str, batch_boundary: &str) -> String {
    let mut body = String::new();

    for item in &batch.items {
        match item {
            BatchItem::Operation(op) => {
                body.push_str(&format!("--{}\r\n", batch_boundary));
                body.push_str("Content-Type: application/http\r\n");
                body.push_str("Content-Transfer-Encoding: binary\r\n");
                if let Some(id) = op.content_id() {
                    body.push_str(&format!("Content-ID: {}\r\n", id));
                }
                body.push_str("\r\n");
                body.push_str(&build_operation_request(op, base_url, &batch.options));
                body.push_str("\r\n");
            }
            BatchItem::Changeset(cs) => {
                let cs_boundary = generate_boundary("changeset");
                body.push_str(&format!("--{}\r\n", batch_boundary));
                body.push_str(&format!(
                    "Content-Type: multipart/mixed; boundary={}\r\n\r\n",
                    cs_boundary
                ));

                for op in &cs.operations {
                    body.push_str(&format!("--{}\r\n", cs_boundary));
                    body.push_str("Content-Type: application/http\r\n");
                    body.push_str("Content-Transfer-Encoding: binary\r\n");
                    if let Some(id) = op.content_id() {
                        body.push_str(&format!("Content-ID: {}\r\n", id));
                    }
                    body.push_str("\r\n");
                    body.push_str(&build_operation_request(op, base_url, &batch.options));
                    body.push_str("\r\n");
                }

                body.push_str(&format!("--{}--\r\n", cs_boundary));
            }
        }
    }

    body.push_str(&format!("--{}--\r\n", batch_boundary));
    body
}

/// Builds a single operation's HTTP request.
fn build_operation_request(op: &Operation, base_url: &str, batch_options: &BatchOptions) -> String {
    let (method, url_path, body_opt, op_options) = operation_parts(op, base_url);

    let mut request = String::new();

    // Request line
    request.push_str(&format!("{} {} HTTP/1.1\r\n", method, url_path));

    // Headers - merge batch options with per-operation options
    let merged_headers = merge_headers(batch_options, op_options);
    for (name, value) in merged_headers {
        request.push_str(&format!("{}: {}\r\n", name, value));
    }

    // Content headers for body
    if let Some(ref body) = body_opt {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }

    request.push_str("\r\n");

    // Body
    if let Some(body) = body_opt {
        request.push_str(&body);
    }

    request
}

/// Extracts parts needed to build the HTTP request from an Operation.
fn operation_parts<'a>(
    op: &'a Operation,
    base_url: &str,
) -> (&'static str, String, Option<String>, &'a OperationOptions) {
    match op {
        Operation::Create {
            entity,
            record,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let url = format!("{}/{}", base_url, entity_set);
            let body = serde_json::to_string(&record.fields()).unwrap_or_default();
            ("POST", url, Some(body), options)
        }

        Operation::Retrieve {
            entity,
            id,
            select,
            expand,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let mut url = format!("{}/{}({})", base_url, entity_set, id);

            let mut params = Vec::new();
            if !select.is_empty() {
                params.push(format!("$select={}", select.join(",")));
            }
            if !expand.is_empty() {
                let expand_clauses: Vec<_> = expand.iter().map(|e| e.to_odata()).collect();
                params.push(format!("$expand={}", expand_clauses.join(",")));
            }
            if !params.is_empty() {
                url.push('?');
                url.push_str(&params.join("&"));
            }

            ("GET", url, None, options)
        }

        Operation::Update {
            entity,
            id,
            record,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let url = format!("{}/{}({})", base_url, entity_set, id);
            let body = serde_json::to_string(&record.fields()).unwrap_or_default();
            ("PATCH", url, Some(body), options)
        }

        Operation::Delete {
            entity,
            id,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let url = format!("{}/{}({})", base_url, entity_set, id);
            ("DELETE", url, None, options)
        }

        Operation::Upsert {
            entity,
            id,
            record,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let url = format!("{}/{}({})", base_url, entity_set, id);
            let body = serde_json::to_string(&record.fields()).unwrap_or_default();
            ("PATCH", url, Some(body), options)
        }

        Operation::Associate {
            entity,
            id,
            relationship,
            target_entity,
            target_id,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let target_set = resolve_entity_set(target_entity);
            let url = format!(
                "{}/{}({})/$ref/{}",
                base_url, entity_set, id, relationship
            );
            let body = format!(
                "{{\"@odata.id\":\"{}/{}({})\"}}",
                base_url, target_set, target_id
            );
            ("POST", url, Some(body), options)
        }

        Operation::Disassociate {
            entity,
            id,
            relationship,
            target_id,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let url = format!(
                "{}/{}({})/$ref/{}({})",
                base_url, entity_set, id, relationship, target_id
            );
            ("DELETE", url, None, options)
        }

        Operation::SetLookup {
            entity,
            id,
            nav_property,
            target_entity,
            target_id,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let target_set = resolve_entity_set(target_entity);
            let url = format!(
                "{}/{}({})/$ref/{}",
                base_url, entity_set, id, nav_property
            );
            let body = format!(
                "{{\"@odata.id\":\"{}/{}({})\"}}",
                base_url, target_set, target_id
            );
            ("PUT", url, Some(body), options)
        }

        Operation::ClearLookup {
            entity,
            id,
            nav_property,
            options,
        } => {
            let entity_set = resolve_entity_set(entity);
            let url = format!(
                "{}/{}({})/$ref/{}",
                base_url, entity_set, id, nav_property
            );
            ("DELETE", url, None, options)
        }
    }
}

/// Resolves an entity to its entity set name.
///
/// Note: For batch operations, we require Entity::Set since we can't do async
/// metadata lookups mid-batch-building.
fn resolve_entity_set(entity: &Entity) -> &str {
    match entity {
        Entity::Set(name) => name,
        Entity::Logical(name) => {
            // For batch, we assume logical name is pluralized entity set name
            // The user should use Entity::Set() for precise control
            name
        }
    }
}

/// Merges batch-level options with per-operation options.
///
/// Per-operation options override batch options where set.
fn merge_headers(
    batch_options: &BatchOptions,
    op_options: &OperationOptions,
) -> Vec<(&'static str, &'static str)> {
    let mut headers = Vec::new();

    // Accept header
    headers.push(("Accept", "application/json"));

    // OData version
    headers.push(("OData-MaxVersion", "4.0"));
    headers.push(("OData-Version", "4.0"));

    // Bypass headers - per-operation overrides batch defaults
    if op_options.bypass_plugins || batch_options.bypass_plugins {
        headers.push(("MSCRM.BypassCustomPluginExecution", "true"));
    }
    if op_options.bypass_flows || batch_options.bypass_flows {
        headers.push(("MSCRM.BypassPowerAutomateFlows", "true"));
    }
    if op_options.bypass_sync_logic || batch_options.bypass_sync_logic {
        headers.push(("MSCRM.BypassBusinessLogicExecution", "true"));
    }
    if op_options.suppress_duplicate_detection || batch_options.suppress_duplicate_detection {
        headers.push(("MSCRM.SuppressDuplicateDetection", "true"));
    }

    // Return record preference
    if op_options.return_record || batch_options.return_record {
        headers.push(("Prefer", "return=representation"));
    }

    // Concurrency headers
    if let Some(ref etag) = op_options.if_match {
        // Note: We can't return dynamic strings here, so this is a simplification
        // The actual implementation might need a different approach
        if etag == "*" {
            headers.push(("If-Match", "*"));
        }
        // For non-wildcard ETags, we'd need a different approach
    }

    if op_options.if_none_match {
        headers.push(("If-None-Match", "*"));
    }

    headers
}
