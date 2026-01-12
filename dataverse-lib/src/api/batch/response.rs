//! Batch response parsing.

use uuid::Uuid;

use crate::error::Error;
use crate::model::Record;

// =============================================================================
// Operation Result
// =============================================================================

/// Result of a single operation in a batch.
#[derive(Debug, Clone)]
pub enum OperationResult {
    /// Create succeeded, returning the new record's ID.
    Created {
        id: Uuid,
        record: Option<Record>,
    },
    /// Retrieve succeeded.
    Retrieved(Record),
    /// Update succeeded.
    Updated {
        record: Option<Record>,
    },
    /// Delete succeeded.
    Deleted,
    /// Upsert succeeded.
    Upserted {
        created: bool,
        id: Uuid,
        record: Option<Record>,
    },
    /// Associate succeeded.
    Associated,
    /// Disassociate succeeded.
    Disassociated,
    /// SetLookup succeeded.
    LookupSet,
    /// ClearLookup succeeded.
    LookupCleared,
}

// =============================================================================
// Batch Operation Error
// =============================================================================

/// Error from a single operation in a batch.
#[derive(Debug, Clone)]
pub struct BatchOperationError {
    /// The Content-ID of the failed operation, if set.
    pub content_id: Option<String>,
    /// The HTTP status code.
    pub status: u16,
    /// The Dataverse error code, if available.
    pub error_code: Option<String>,
    /// The error message.
    pub message: String,
}

impl std::fmt::Display for BatchOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref code) = self.error_code {
            write!(f, "[{}] {}: {}", self.status, code, self.message)
        } else {
            write!(f, "[{}] {}", self.status, self.message)
        }
    }
}

impl std::error::Error for BatchOperationError {}

// =============================================================================
// Batch Item Result
// =============================================================================

/// Result of a single batch item (operation or changeset).
#[derive(Debug)]
pub enum BatchItemResult {
    /// Result of a standalone operation.
    Operation(Result<OperationResult, BatchOperationError>),
    /// Result of a changeset (all operations succeed or fail together).
    Changeset(Result<Vec<OperationResult>, BatchOperationError>),
}

// =============================================================================
// Batch Results
// =============================================================================

/// Result of a batch execution.
#[derive(Debug)]
pub struct BatchResults {
    results: Vec<BatchItemResult>,
}

impl BatchResults {
    /// Parses a multipart batch response.
    pub fn parse(response_body: &str, boundary: &str) -> Result<Self, Error> {
        let mut results = Vec::new();
        let boundary_marker = format!("--{}", boundary);

        // Split by boundary
        let parts: Vec<&str> = response_body
            .split(&boundary_marker)
            .filter(|s| !s.trim().is_empty() && !s.trim().starts_with("--"))
            .collect();

        for part in parts {
            // Check if this is a changeset (nested multipart)
            if part.contains("multipart/mixed") {
                // Extract nested boundary
                if let Some(nested_boundary) = extract_boundary_from_header(part) {
                    let changeset_result = parse_changeset(part, &nested_boundary)?;
                    results.push(BatchItemResult::Changeset(changeset_result));
                }
            } else {
                // Single operation
                let op_result = parse_operation_response(part)?;
                results.push(BatchItemResult::Operation(op_result));
            }
        }

        Ok(BatchResults { results })
    }

    /// Returns the number of items in the batch result.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Returns true if there are no results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Checks if all operations succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(|r| match r {
            BatchItemResult::Operation(Ok(_)) => true,
            BatchItemResult::Changeset(Ok(_)) => true,
            _ => false,
        })
    }

    /// Iterator over results.
    pub fn iter(&self) -> impl Iterator<Item = &BatchItemResult> {
        self.results.iter()
    }

    /// Gets an operation result by index.
    pub fn operation(&self, index: usize) -> Option<&Result<OperationResult, BatchOperationError>> {
        match self.results.get(index)? {
            BatchItemResult::Operation(r) => Some(r),
            _ => None,
        }
    }

    /// Gets changeset results by index.
    pub fn changeset(
        &self,
        index: usize,
    ) -> Option<&Result<Vec<OperationResult>, BatchOperationError>> {
        match self.results.get(index)? {
            BatchItemResult::Changeset(r) => Some(r),
            _ => None,
        }
    }

    /// Returns the results as a vector.
    pub fn into_vec(self) -> Vec<BatchItemResult> {
        self.results
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// Extracts the boundary from a Content-Type header.
fn extract_boundary_from_header(header_section: &str) -> Option<String> {
    for line in header_section.lines() {
        if line.to_lowercase().contains("content-type") && line.contains("boundary=") {
            if let Some(idx) = line.find("boundary=") {
                let boundary = line[idx + 9..].trim();
                // Remove quotes if present
                let boundary = boundary.trim_matches('"');
                return Some(boundary.to_string());
            }
        }
    }
    None
}

/// Parses a changeset's operations.
fn parse_changeset(
    changeset_body: &str,
    boundary: &str,
) -> Result<Result<Vec<OperationResult>, BatchOperationError>, Error> {
    let boundary_marker = format!("--{}", boundary);
    let mut results = Vec::new();

    let parts: Vec<&str> = changeset_body
        .split(&boundary_marker)
        .filter(|s| !s.trim().is_empty() && !s.trim().starts_with("--"))
        .collect();

    for part in parts {
        // Skip the Content-Type header part
        if part.contains("Content-Type: multipart/mixed") {
            continue;
        }

        match parse_operation_response(part)? {
            Ok(result) => results.push(result),
            Err(e) => {
                // If any operation in changeset fails, the whole changeset fails
                return Ok(Err(e));
            }
        }
    }

    Ok(Ok(results))
}

/// Parses a single operation's HTTP response.
fn parse_operation_response(
    response: &str,
) -> Result<Result<OperationResult, BatchOperationError>, Error> {
    // Find the HTTP status line
    let lines: Vec<&str> = response.lines().collect();

    // Find the HTTP/1.1 status line
    let status_line = lines
        .iter()
        .find(|line| line.starts_with("HTTP/1.1"))
        .ok_or_else(|| {
            Error::InvalidOperation("Batch response missing HTTP status line".to_string())
        })?;

    // Parse status code
    let parts: Vec<&str> = status_line.split_whitespace().collect();
    let status: u16 = parts
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    // Find where headers end and body begins (empty line)
    let body_start = response.find("\r\n\r\n").or_else(|| response.find("\n\n"));
    let body = body_start
        .map(|idx| response[idx..].trim())
        .unwrap_or("");

    // Check for success (2xx status codes)
    if (200..300).contains(&status) {
        let result = parse_success_response(status, body)?;
        Ok(Ok(result))
    } else {
        let error = parse_error_response(status, body, response);
        Ok(Err(error))
    }
}

/// Parses a successful operation response.
fn parse_success_response(status: u16, body: &str) -> Result<OperationResult, Error> {
    match status {
        201 => {
            // Created - try to extract ID from response or OData-EntityId header
            if body.is_empty() {
                // No body, need to extract ID from headers (simplified)
                Ok(OperationResult::Created {
                    id: Uuid::nil(),
                    record: None,
                })
            } else {
                // Parse JSON body
                let record: Record = serde_json::from_str(body).map_err(|e| {
                    Error::InvalidOperation(format!("Failed to parse created record: {}", e))
                })?;
                let id = record.id().unwrap_or(Uuid::nil());
                Ok(OperationResult::Created {
                    id,
                    record: Some(record),
                })
            }
        }
        200 => {
            // OK - could be retrieve, update with return, etc.
            if body.is_empty() {
                Ok(OperationResult::Updated { record: None })
            } else {
                let record: Record = serde_json::from_str(body).map_err(|e| {
                    Error::InvalidOperation(format!("Failed to parse record: {}", e))
                })?;
                Ok(OperationResult::Retrieved(record))
            }
        }
        204 => {
            // No Content - successful delete, update without return, etc.
            Ok(OperationResult::Deleted)
        }
        _ => Ok(OperationResult::Updated { record: None }),
    }
}

/// Parses an error response.
fn parse_error_response(status: u16, body: &str, full_response: &str) -> BatchOperationError {
    // Try to parse OData error format
    let (error_code, message) = if !body.is_empty() {
        parse_odata_error(body)
    } else {
        (None, format!("HTTP {} error", status))
    };

    // Try to extract Content-ID from headers
    let content_id = full_response
        .lines()
        .find(|line| line.to_lowercase().starts_with("content-id:"))
        .map(|line| line[11..].trim().to_string());

    BatchOperationError {
        content_id,
        status,
        error_code,
        message,
    }
}

/// Parses an OData error response body.
fn parse_odata_error(body: &str) -> (Option<String>, String) {
    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(error) = json.get("error") {
            let code = error
                .get("code")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let message = error
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            return (code, message);
        }
    }
    (None, body.to_string())
}

/// Extracts the boundary from a response Content-Type header value.
pub fn extract_boundary(content_type: &str) -> Option<String> {
    content_type
        .split(';')
        .find(|part| part.trim().starts_with("boundary="))
        .map(|part| {
            part.trim()
                .strip_prefix("boundary=")
                .unwrap_or("")
                .trim_matches('"')
                .to_string()
        })
}
