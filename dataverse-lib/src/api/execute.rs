//! Operation execution logic
//!
//! This module contains the HTTP execution logic for CRUD operations.

use std::time::Duration;

use reqwest::Method;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use serde_json::json;
use uuid::Uuid;

use super::crud::CreateResult;
use super::crud::Expand;
use super::crud::Operation;
use super::crud::OperationOptions;
use super::crud::UpsertResult;
use super::aggregate::AggregateBuilder;
use super::query::fetchxml::FetchBuilder;
use super::query::odata::QueryBuilder;
use crate::DataverseClient;
use crate::error::ApiError;
use crate::error::Error;
use crate::model::Entity;
use crate::model::Record;
use crate::response::Response;

impl DataverseClient {
    /// Executes any operation.
    ///
    /// This is the universal execution method that can run any [`Operation`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dataverse_lib::api::Op;
    ///
    /// let op = Op::create(Entity::set("accounts"), record).build();
    /// let result = client.execute(op).await?;
    /// ```
    pub async fn execute(&self, operation: impl Into<Operation>) -> Result<OperationResult, Error> {
        let operation = operation.into();
        match operation {
            Operation::Create {
                entity,
                record,
                options,
            } => {
                let result = self.execute_create(entity, record, options).await?;
                Ok(OperationResult::Create(result))
            }
            Operation::Retrieve {
                entity,
                id,
                select,
                expand,
                options,
            } => {
                let result = self.execute_retrieve(entity, id, select, expand, options).await?;
                Ok(OperationResult::Retrieve(result))
            }
            Operation::Update {
                entity,
                id,
                record,
                options,
            } => {
                let result = self.execute_update(entity, id, record, options).await?;
                Ok(OperationResult::Update(result))
            }
            Operation::Delete {
                entity,
                id,
                options,
            } => {
                self.execute_delete(entity, id, options).await?;
                Ok(OperationResult::Delete)
            }
            Operation::Upsert {
                entity,
                id,
                record,
                options,
            } => {
                let result = self.execute_upsert(entity, id, record, options).await?;
                Ok(OperationResult::Upsert(result))
            }
            Operation::Associate {
                entity,
                id,
                relationship,
                target_entity,
                target_id,
                options,
            } => {
                self.execute_associate(
                    entity,
                    id,
                    &relationship,
                    target_entity,
                    target_id,
                    options,
                )
                .await?;
                Ok(OperationResult::Associate)
            }
            Operation::Disassociate {
                entity,
                id,
                relationship,
                target_id,
                options,
            } => {
                self.execute_disassociate(entity, id, &relationship, target_id, options)
                    .await?;
                Ok(OperationResult::Disassociate)
            }
            Operation::SetLookup {
                entity,
                id,
                nav_property,
                target_entity,
                target_id,
                options,
            } => {
                self.execute_set_lookup(
                    entity,
                    id,
                    &nav_property,
                    target_entity,
                    target_id,
                    options,
                )
                .await?;
                Ok(OperationResult::SetLookup)
            }
            Operation::ClearLookup {
                entity,
                id,
                nav_property,
                options,
            } => {
                self.execute_clear_lookup(entity, id, &nav_property, options)
                    .await?;
                Ok(OperationResult::ClearLookup)
            }
        }
    }

    // =========================================================================
    // Individual operation execution
    // =========================================================================

    async fn execute_create(
        &self,
        entity: Entity,
        record: Record,
        options: OperationOptions,
    ) -> Result<CreateResult, Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let url = self.build_url(&format!("/{}", entity_set));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        if options.return_record {
            headers.insert("Prefer", HeaderValue::from_static("return=representation"));
            // TODO: Add $select query parameter when options.select is non-empty
            // Requires OData URL builder - see dataverse-lib.md plan
        }

        let body = serde_json::to_string(&record).map_err(|e| Error::Serialization(e))?;

        let response = self
            .request(Method::POST, &url, headers, Some(body))
            .await?;

        if options.return_record {
            // 201 Created with body
            let record: Record = response.json().await.map_err(ApiError::from)?;
            Ok(CreateResult::Record(record))
        } else {
            // 204 No Content with OData-EntityId header
            let entity_id = response
                .headers()
                .get("OData-EntityId")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| extract_guid_from_entity_id(s))
                .ok_or_else(|| {
                    Error::Api(ApiError::Parse {
                        message: "Missing or invalid OData-EntityId header".to_string(),
                        body: None,
                    })
                })?;
            Ok(CreateResult::Id(entity_id))
        }
    }

    async fn execute_retrieve(
        &self,
        entity: Entity,
        id: Uuid,
        select: Vec<String>,
        expand: Vec<Expand>,
        options: OperationOptions,
    ) -> Result<Response<Record>, Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let mut url = format!("/{}({})", entity_set, id);

        // Build query parameters
        let mut query_parts = Vec::new();

        if !select.is_empty() {
            query_parts.push(format!("$select={}", select.join(",")));
        }

        if !expand.is_empty() {
            let expand_str: Vec<String> = expand.iter().map(|e| e.to_odata_string()).collect();
            query_parts.push(format!("$expand={}", expand_str.join(",")));
        }

        if !query_parts.is_empty() {
            url.push('?');
            url.push_str(&query_parts.join("&"));
        }

        let full_url = self.build_url(&url);
        let mut headers = self.default_headers();
        headers.insert(
            "Prefer",
            HeaderValue::from_static("odata.include-annotations=\"*\""),
        );
        self.apply_options_headers(&mut headers, &options);

        let response = self.request(Method::GET, &full_url, headers, None).await?;
        let record: Record = response.json().await.map_err(ApiError::from)?;

        Ok(Response::fresh(record))
    }

    async fn execute_update(
        &self,
        entity: Entity,
        id: Uuid,
        record: Record,
        options: OperationOptions,
    ) -> Result<Option<Record>, Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let url = self.build_url(&format!("/{}({})", entity_set, id));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        if let Some(ref etag) = options.if_match {
            let header_value = HeaderValue::from_str(etag)
                .map_err(|_| Error::InvalidOperation(format!("Invalid etag value: {}", etag)))?;
            headers.insert("If-Match", header_value);
        }

        if options.return_record {
            headers.insert("Prefer", HeaderValue::from_static("return=representation"));
        }

        let body = serde_json::to_string(&record).map_err(|e| Error::Serialization(e))?;

        let response = self
            .request(Method::PATCH, &url, headers, Some(body))
            .await?;

        if options.return_record {
            let record: Record = response.json().await.map_err(ApiError::from)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    async fn execute_delete(
        &self,
        entity: Entity,
        id: Uuid,
        options: OperationOptions,
    ) -> Result<(), Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let url = self.build_url(&format!("/{}({})", entity_set, id));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        if let Some(ref etag) = options.if_match {
            let header_value = HeaderValue::from_str(etag)
                .map_err(|_| Error::InvalidOperation(format!("Invalid etag value: {}", etag)))?;
            headers.insert("If-Match", header_value);
        }

        self.request(Method::DELETE, &url, headers, None).await?;
        Ok(())
    }

    async fn execute_upsert(
        &self,
        entity: Entity,
        id: Uuid,
        record: Record,
        options: OperationOptions,
    ) -> Result<UpsertResult, Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let url = self.build_url(&format!("/{}({})", entity_set, id));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        if options.if_none_match {
            headers.insert("If-None-Match", HeaderValue::from_static("*"));
        } else if let Some(ref etag) = options.if_match {
            let header_value = HeaderValue::from_str(etag)
                .map_err(|_| Error::InvalidOperation(format!("Invalid etag value: {}", etag)))?;
            headers.insert("If-Match", header_value);
        }

        if options.return_record {
            headers.insert("Prefer", HeaderValue::from_static("return=representation"));
        }

        let body = serde_json::to_string(&record).map_err(|e| Error::Serialization(e))?;

        let response = self
            .request(Method::PATCH, &url, headers, Some(body))
            .await?;

        let status = response.status();
        if status == StatusCode::CREATED {
            // New record created
            if options.return_record {
                let record: Record = response.json().await.map_err(ApiError::from)?;
                Ok(UpsertResult::Created(CreateResult::Record(record)))
            } else {
                let entity_id = response
                    .headers()
                    .get("OData-EntityId")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| extract_guid_from_entity_id(s))
                    .unwrap_or(id); // Fall back to provided id
                Ok(UpsertResult::Created(CreateResult::Id(entity_id)))
            }
        } else {
            // Existing record updated
            if options.return_record {
                let record: Record = response.json().await.map_err(ApiError::from)?;
                Ok(UpsertResult::Updated { id, record: Some(record) })
            } else {
                Ok(UpsertResult::Updated { id, record: None })
            }
        }
    }

    async fn execute_associate(
        &self,
        entity: Entity,
        id: Uuid,
        relationship: &str,
        target_entity: Entity,
        target_id: Uuid,
        options: OperationOptions,
    ) -> Result<(), Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let target_set = self.resolve_entity(&target_entity).await?;

        // The relationship name is used as the collection-valued navigation property
        // The actual URL format is: POST /{entity}({id})/{relationship}/$ref
        let url = self.build_url(&format!("/{}({})/{}/$ref", entity_set, id, relationship));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        let body = json!({
            "@odata.id": self.build_url(&format!("/{}({})", target_set, target_id))
        });

        self.request(Method::POST, &url, headers, Some(body.to_string()))
            .await?;
        Ok(())
    }

    async fn execute_disassociate(
        &self,
        entity: Entity,
        id: Uuid,
        relationship: &str,
        target_id: Uuid,
        options: OperationOptions,
    ) -> Result<(), Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let url = self.build_url(&format!(
            "/{}({})/{}({})/$ref",
            entity_set, id, relationship, target_id
        ));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        self.request(Method::DELETE, &url, headers, None).await?;
        Ok(())
    }

    async fn execute_set_lookup(
        &self,
        entity: Entity,
        id: Uuid,
        nav_property: &str,
        target_entity: Entity,
        target_id: Uuid,
        options: OperationOptions,
    ) -> Result<(), Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let target_set = self.resolve_entity(&target_entity).await?;
        let url = self.build_url(&format!("/{}({})/{}/$ref", entity_set, id, nav_property));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        let body = json!({
            "@odata.id": self.build_url(&format!("/{}({})", target_set, target_id))
        });

        self.request(Method::PUT, &url, headers, Some(body.to_string()))
            .await?;
        Ok(())
    }

    async fn execute_clear_lookup(
        &self,
        entity: Entity,
        id: Uuid,
        nav_property: &str,
        options: OperationOptions,
    ) -> Result<(), Error> {
        let entity_set = self.resolve_entity(&entity).await?;
        let url = self.build_url(&format!("/{}({})/{}/$ref", entity_set, id, nav_property));

        let mut headers = self.default_headers();
        self.apply_options_headers(&mut headers, &options);

        self.request(Method::DELETE, &url, headers, None).await?;
        Ok(())
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Resolves an Entity to its entity set name for use in API URLs.
    ///
    /// - For `Entity::Set`, returns the name directly.
    /// - For `Entity::Logical`, fetches metadata to resolve the entity set name.
    async fn resolve_entity(&self, entity: &Entity) -> Result<String, Error> {
        match entity {
            Entity::Set(name) => Ok(name.clone()),
            Entity::Logical(logical_name) => self.resolve_entity_set_name(logical_name).await,
        }
    }

    fn build_url(&self, path: &str) -> String {
        format!(
            "{}/api/data/{}{}",
            self.inner.base_url.trim_end_matches('/'),
            self.inner.api_version,
            path
        )
    }

    fn default_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("OData-MaxVersion", HeaderValue::from_static("4.0"));
        headers.insert("OData-Version", HeaderValue::from_static("4.0"));
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers
    }

    fn apply_options_headers(&self, headers: &mut HeaderMap, options: &OperationOptions) {
        for (name, value) in options.bypass_headers() {
            if let Ok(header_value) = HeaderValue::from_str(value) {
                headers.insert(name, header_value);
            }
        }
    }

    /// Makes an HTTP request with rate limiting and retry logic.
    ///
    /// This is the low-level request method used by all API operations.
    pub(crate) async fn request(
        &self,
        method: Method,
        url: &str,
        headers: impl Into<Option<HeaderMap>>,
        body: Option<String>,
    ) -> Result<reqwest::Response, Error> {
        let headers = headers.into().unwrap_or_default();
        // Acquire concurrency permit (held for entire request lifecycle including retries)
        let _permit = self.inner.concurrency_limiter.acquire().await;

        let retry_config = &self.inner.retry_config;
        let mut attempts = 0;
        let mut delay = retry_config.initial_delay;

        loop {
            // Acquire rate limit slot
            self.inner.rate_limiter.acquire().await;

            // Send request
            let result = self
                .send_request_inner(method.clone(), url, headers.clone(), body.clone())
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();

                    // Handle 429 Too Many Requests
                    if status.as_u16() == 429 {
                        if !retry_config.retry_on_429 || attempts >= retry_config.max_retries {
                            let retry_after = parse_retry_after(&response);
                            return Err(Error::RateLimit { retry_after });
                        }

                        let wait = parse_retry_after(&response).unwrap_or(delay);
                        tokio::time::sleep(wait).await;
                        attempts += 1;
                        continue;
                    }

                    // Handle 5xx server errors
                    if status.is_server_error() {
                        if !retry_config.retry_on_5xx || attempts >= retry_config.max_retries {
                            let status_code = status.as_u16();
                            let body = response.text().await.unwrap_or_default();
                            return Err(Error::Api(ApiError::Http {
                                status: status_code,
                                message: body,
                                code: None,
                                inner: None,
                            }));
                        }

                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(retry_config.max_delay);
                        attempts += 1;
                        continue;
                    }

                    // Success or client error (4xx except 429)
                    if status.is_success() {
                        return Ok(response);
                    } else {
                        let status_code = status.as_u16();
                        let body = response.text().await.unwrap_or_default();
                        return Err(Error::Api(ApiError::Http {
                            status: status_code,
                            message: body,
                            code: None,
                            inner: None,
                        }));
                    }
                }
                Err(e) => {
                    // Handle network errors
                    let is_network = matches!(&e, Error::Api(ApiError::Network(_)));

                    if is_network
                        && retry_config.retry_on_network
                        && attempts < retry_config.max_retries
                    {
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(retry_config.max_delay);
                        attempts += 1;
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }

    /// Inner request method without retry logic.
    async fn send_request_inner(
        &self,
        method: Method,
        url: &str,
        headers: HeaderMap,
        body: Option<String>,
    ) -> Result<reqwest::Response, Error> {
        let token = self
            .inner
            .token_provider
            .get_token(&self.inner.base_url)
            .await?;

        let mut request = self
            .inner
            .http_client
            .request(method, url)
            .headers(headers)
            .bearer_auth(&token.access_token);

        if let Some(timeout) = self.inner.timeout {
            request = request.timeout(timeout);
        }

        if let Some(body) = body {
            request = request.body(body);
        }

        request.send().await.map_err(|e| Error::Api(ApiError::from(e)))
    }
}

/// Parses the Retry-After header value (seconds).
fn parse_retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get("Retry-After")?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// Result of executing an operation.
#[derive(Debug)]
pub enum OperationResult {
    Create(CreateResult),
    Retrieve(Response<Record>),
    Update(Option<Record>),
    Delete,
    Upsert(UpsertResult),
    Associate,
    Disassociate,
    SetLookup,
    ClearLookup,
}

impl OperationResult {
    /// Returns the created ID if this was a Create operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the created record doesn't contain an ID.
    pub fn created_id(&self) -> Result<Option<Uuid>, Error> {
        match self {
            OperationResult::Create(result) => Ok(Some(result.id()?)),
            _ => Ok(None),
        }
    }

    /// Returns the record if this was a Retrieve operation.
    pub fn record(&self) -> Option<&Record> {
        match self {
            OperationResult::Retrieve(response) => Some(response.data()),
            OperationResult::Create(CreateResult::Record(r)) => Some(r),
            OperationResult::Update(Some(r)) => Some(r),
            OperationResult::Upsert(UpsertResult::Created(CreateResult::Record(r))) => Some(r),
            OperationResult::Upsert(UpsertResult::Updated { record: Some(r), .. }) => Some(r),
            _ => None,
        }
    }
}

/// Extracts a GUID from an OData-EntityId header value.
///
/// The header looks like: `https://org.crm.dynamics.com/api/data/v9.2/accounts(00000000-0000-0000-0000-000000000001)`
fn extract_guid_from_entity_id(entity_id: &str) -> Option<Uuid> {
    // Find the last occurrence of '(' and ')'
    let start = entity_id.rfind('(')? + 1;
    let end = entity_id.rfind(')')?;
    let guid_str = &entity_id[start..end];
    Uuid::parse_str(guid_str).ok()
}

// =============================================================================
// Convenience CRUD methods on DataverseClient
// =============================================================================

impl DataverseClient {
    /// Creates a new record.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Simple create
    /// let id = client.create(Entity::set("accounts"), record).await?;
    ///
    /// // With options
    /// let created = client.create(Entity::set("accounts"), record)
    ///     .return_record()
    ///     .bypass_plugins()
    ///     .await?;
    /// ```
    pub fn create(&self, entity: Entity, record: Record) -> ClientCreateBuilder<'_> {
        ClientCreateBuilder {
            client: self,
            entity,
            record,
            options: OperationOptions::default(),
        }
    }

    /// Retrieves a record by ID.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let record = client.retrieve(Entity::set("accounts"), id)
    ///     .select(&["name", "revenue"])
    ///     .await?;
    /// ```
    pub fn retrieve(&self, entity: Entity, id: Uuid) -> ClientRetrieveBuilder<'_> {
        ClientRetrieveBuilder {
            client: self,
            entity,
            id,
            select: Vec::new(),
            expand: Vec::new(),
            options: OperationOptions::default(),
        }
    }

    /// Updates an existing record.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.update(Entity::set("accounts"), id, record).await?;
    /// ```
    pub fn update(&self, entity: Entity, id: Uuid, record: Record) -> ClientUpdateBuilder<'_> {
        ClientUpdateBuilder {
            client: self,
            entity,
            id,
            record,
            options: OperationOptions::default(),
        }
    }

    /// Deletes a record.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.delete(Entity::set("accounts"), id).await?;
    /// ```
    pub fn delete(&self, entity: Entity, id: Uuid) -> ClientDeleteBuilder<'_> {
        ClientDeleteBuilder {
            client: self,
            entity,
            id,
            options: OperationOptions::default(),
        }
    }

    /// Upserts (creates or updates) a record.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = client.upsert(Entity::set("accounts"), id, record).await?;
    /// if result.is_created() {
    ///     println!("Created new record");
    /// }
    /// ```
    pub fn upsert(&self, entity: Entity, id: Uuid, record: Record) -> ClientUpsertBuilder<'_> {
        ClientUpsertBuilder {
            client: self,
            entity,
            id,
            record,
            options: OperationOptions::default(),
        }
    }

    /// Associates two records via an N:N relationship.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.associate(
    ///     Entity::set("accounts"), account_id,
    ///     "contact_customer_accounts",
    ///     Entity::set("contacts"), contact_id,
    /// ).await?;
    /// ```
    pub fn associate(
        &self,
        entity: Entity,
        id: Uuid,
        relationship: impl Into<String>,
        target_entity: Entity,
        target_id: Uuid,
    ) -> ClientAssociateBuilder<'_> {
        ClientAssociateBuilder {
            client: self,
            entity,
            id,
            relationship: relationship.into(),
            target_entity,
            target_id,
            options: OperationOptions::default(),
        }
    }

    /// Disassociates two records from an N:N relationship.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.disassociate(
    ///     Entity::set("accounts"), account_id,
    ///     "contact_customer_accounts",
    ///     contact_id,
    /// ).await?;
    /// ```
    pub fn disassociate(
        &self,
        entity: Entity,
        id: Uuid,
        relationship: impl Into<String>,
        target_id: Uuid,
    ) -> ClientDisassociateBuilder<'_> {
        ClientDisassociateBuilder {
            client: self,
            entity,
            id,
            relationship: relationship.into(),
            target_id,
            options: OperationOptions::default(),
        }
    }

    /// Sets a single-valued navigation property (lookup field).
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.set_lookup(
    ///     Entity::set("accounts"), account_id,
    ///     "primarycontactid",
    ///     Entity::set("contacts"), contact_id,
    /// ).await?;
    /// ```
    pub fn set_lookup(
        &self,
        entity: Entity,
        id: Uuid,
        nav_property: impl Into<String>,
        target_entity: Entity,
        target_id: Uuid,
    ) -> ClientSetLookupBuilder<'_> {
        ClientSetLookupBuilder {
            client: self,
            entity,
            id,
            nav_property: nav_property.into(),
            target_entity,
            target_id,
            options: OperationOptions::default(),
        }
    }

    /// Clears a single-valued navigation property (lookup field).
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.clear_lookup(
    ///     Entity::set("accounts"), account_id,
    ///     "primarycontactid",
    /// ).await?;
    /// ```
    pub fn clear_lookup(
        &self,
        entity: Entity,
        id: Uuid,
        nav_property: impl Into<String>,
    ) -> ClientClearLookupBuilder<'_> {
        ClientClearLookupBuilder {
            client: self,
            entity,
            id,
            nav_property: nav_property.into(),
            options: OperationOptions::default(),
        }
    }

    /// Creates an OData query for the specified entity.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dataverse_lib::api::query::{Filter, OrderBy};
    ///
    /// let mut pages = client.query(Entity::logical("account"))
    ///     .select(&["name", "revenue"])
    ///     .filter(Filter::gt("revenue", 1000000))
    ///     .order_by(OrderBy::desc("revenue"))
    ///     .into_async_iter();
    ///
    /// while let Some(page) = pages.next().await {
    ///     let page = page?;
    ///     for record in page.records() {
    ///         println!("{:?}", record);
    ///     }
    /// }
    /// ```
    pub fn query(&self, entity: Entity) -> QueryBuilder<'_> {
        QueryBuilder::new(self, entity)
    }

    /// Creates a FetchXML query for the specified entity.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// FetchXML provides more advanced querying capabilities than OData,
    /// including complex aggregations and multi-level joins.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dataverse_lib::api::query::{Filter, OrderBy};
    /// use dataverse_lib::api::query::fetchxml::LinkType;
    ///
    /// let mut pages = client.fetch(Entity::logical("account"))
    ///     .select(&["name", "revenue"])
    ///     .filter(Filter::gt("revenue", 1000000))
    ///     .link_entity("contact", "contactid", "primarycontactid", |link| {
    ///         link.alias("pc")
    ///             .select(&["fullname"])
    ///             .link_type(LinkType::Outer)
    ///     })
    ///     .into_async_iter();
    ///
    /// while let Some(page) = pages.next().await {
    ///     let page = page?;
    ///     for record in page.records() {
    ///         println!("{:?}", record);
    ///     }
    /// }
    /// ```
    pub fn fetch(&self, entity: Entity) -> FetchBuilder<'_> {
        FetchBuilder::new(self, entity)
    }

    /// Creates an aggregation query for the specified entity.
    ///
    /// Returns a builder that can be configured and executed.
    ///
    /// Aggregation queries use FetchXML internally, as it's the only
    /// query language in Dataverse that supports aggregations.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use dataverse_lib::api::query::Filter;
    ///
    /// let results = client.aggregate(Entity::logical("opportunity"))
    ///     .group_by("ownerid", "owner")
    ///     .sum("estimatedvalue", "total_value")
    ///     .count("opportunityid", "count")
    ///     .filter(Filter::eq("statecode", 0))
    ///     .execute()
    ///     .await?;
    ///
    /// for result in results {
    ///     println!("Owner: {:?}, Total: {:?}, Count: {:?}",
    ///         result.get("owner"),
    ///         result.get("total_value"),
    ///         result.get("count"));
    /// }
    /// ```
    pub fn aggregate(&self, entity: Entity) -> AggregateBuilder<'_> {
        AggregateBuilder::new(self, entity)
    }
}

// =============================================================================
// Client-bound builders (with IntoFuture for .await)
// =============================================================================

/// Builder for create operations bound to a client.
pub struct ClientCreateBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    record: Record,
    options: OperationOptions,
}

impl<'a> ClientCreateBuilder<'a> {
    /// Request the created record in the response.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    /// Sets the fields to include in the returned record.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.options.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }

    /// Skip duplicate detection.
    pub fn suppress_duplicate_detection(mut self) -> Self {
        self.options.suppress_duplicate_detection = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientCreateBuilder<'a> {
    type Output = Result<CreateResult, Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_create(self.entity, self.record, self.options)
                .await
        })
    }
}

impl<'a> From<ClientCreateBuilder<'a>> for Operation {
    fn from(builder: ClientCreateBuilder<'a>) -> Self {
        Operation::Create {
            entity: builder.entity,
            record: builder.record,
            options: builder.options,
        }
    }
}

/// Builder for retrieve operations bound to a client.
pub struct ClientRetrieveBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    select: Vec<String>,
    expand: Vec<Expand>,
    options: OperationOptions,
}

impl<'a> ClientRetrieveBuilder<'a> {
    /// Sets the fields to retrieve.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Adds a navigation property to expand.
    pub fn expand(
        mut self,
        nav_property: impl Into<String>,
        configure: impl FnOnce(Expand) -> Expand,
    ) -> Self {
        let expand = configure(Expand::new(nav_property));
        self.expand.push(expand);
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientRetrieveBuilder<'a> {
    type Output = Result<Response<Record>, Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_retrieve(self.entity, self.id, self.select, self.expand, self.options)
                .await
        })
    }
}

impl<'a> From<ClientRetrieveBuilder<'a>> for Operation {
    fn from(builder: ClientRetrieveBuilder<'a>) -> Self {
        Operation::Retrieve {
            entity: builder.entity,
            id: builder.id,
            select: builder.select,
            expand: builder.expand,
            options: builder.options,
        }
    }
}

/// Builder for update operations bound to a client.
pub struct ClientUpdateBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    record: Record,
    options: OperationOptions,
}

impl<'a> ClientUpdateBuilder<'a> {
    /// Request the updated record in the response.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    /// Sets the fields to include in the returned record.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.options.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Sets the ETag for optimistic concurrency.
    pub fn if_match(mut self, etag: impl Into<String>) -> Self {
        self.options.if_match = Some(etag.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientUpdateBuilder<'a> {
    type Output = Result<Option<Record>, Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_update(self.entity, self.id, self.record, self.options)
                .await
        })
    }
}

impl<'a> From<ClientUpdateBuilder<'a>> for Operation {
    fn from(builder: ClientUpdateBuilder<'a>) -> Self {
        Operation::Update {
            entity: builder.entity,
            id: builder.id,
            record: builder.record,
            options: builder.options,
        }
    }
}

/// Builder for delete operations bound to a client.
pub struct ClientDeleteBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    options: OperationOptions,
}

impl<'a> ClientDeleteBuilder<'a> {
    /// Sets the ETag for optimistic concurrency.
    pub fn if_match(mut self, etag: impl Into<String>) -> Self {
        self.options.if_match = Some(etag.into());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientDeleteBuilder<'a> {
    type Output = Result<(), Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_delete(self.entity, self.id, self.options)
                .await
        })
    }
}

impl<'a> From<ClientDeleteBuilder<'a>> for Operation {
    fn from(builder: ClientDeleteBuilder<'a>) -> Self {
        Operation::Delete {
            entity: builder.entity,
            id: builder.id,
            options: builder.options,
        }
    }
}

/// Builder for upsert operations bound to a client.
pub struct ClientUpsertBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    record: Record,
    options: OperationOptions,
}

impl<'a> ClientUpsertBuilder<'a> {
    /// Request the created/updated record in the response.
    pub fn return_record(mut self) -> Self {
        self.options.return_record = true;
        self
    }

    /// Sets the fields to include in the returned record.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.options.select = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Only create if doesn't exist (If-None-Match: *).
    pub fn if_none_match(mut self) -> Self {
        self.options.if_none_match = true;
        self
    }

    /// Only update if exists (If-Match: *).
    pub fn if_match_any(mut self) -> Self {
        self.options.if_match = Some("*".to_string());
        self
    }

    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }

    /// Skip synchronous business logic.
    pub fn bypass_sync_logic(mut self) -> Self {
        self.options.bypass_sync_logic = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientUpsertBuilder<'a> {
    type Output = Result<UpsertResult, Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_upsert(self.entity, self.id, self.record, self.options)
                .await
        })
    }
}

impl<'a> From<ClientUpsertBuilder<'a>> for Operation {
    fn from(builder: ClientUpsertBuilder<'a>) -> Self {
        Operation::Upsert {
            entity: builder.entity,
            id: builder.id,
            record: builder.record,
            options: builder.options,
        }
    }
}

/// Builder for associate operations bound to a client.
pub struct ClientAssociateBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    relationship: String,
    target_entity: Entity,
    target_id: Uuid,
    options: OperationOptions,
}

impl<'a> ClientAssociateBuilder<'a> {
    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientAssociateBuilder<'a> {
    type Output = Result<(), Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_associate(
                    self.entity,
                    self.id,
                    &self.relationship,
                    self.target_entity,
                    self.target_id,
                    self.options,
                )
                .await
        })
    }
}

impl<'a> From<ClientAssociateBuilder<'a>> for Operation {
    fn from(builder: ClientAssociateBuilder<'a>) -> Self {
        Operation::Associate {
            entity: builder.entity,
            id: builder.id,
            relationship: builder.relationship,
            target_entity: builder.target_entity,
            target_id: builder.target_id,
            options: builder.options,
        }
    }
}

/// Builder for disassociate operations bound to a client.
pub struct ClientDisassociateBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    relationship: String,
    target_id: Uuid,
    options: OperationOptions,
}

impl<'a> ClientDisassociateBuilder<'a> {
    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientDisassociateBuilder<'a> {
    type Output = Result<(), Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_disassociate(
                    self.entity,
                    self.id,
                    &self.relationship,
                    self.target_id,
                    self.options,
                )
                .await
        })
    }
}

impl<'a> From<ClientDisassociateBuilder<'a>> for Operation {
    fn from(builder: ClientDisassociateBuilder<'a>) -> Self {
        Operation::Disassociate {
            entity: builder.entity,
            id: builder.id,
            relationship: builder.relationship,
            target_id: builder.target_id,
            options: builder.options,
        }
    }
}

/// Builder for set_lookup operations bound to a client.
pub struct ClientSetLookupBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    nav_property: String,
    target_entity: Entity,
    target_id: Uuid,
    options: OperationOptions,
}

impl<'a> ClientSetLookupBuilder<'a> {
    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientSetLookupBuilder<'a> {
    type Output = Result<(), Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_set_lookup(
                    self.entity,
                    self.id,
                    &self.nav_property,
                    self.target_entity,
                    self.target_id,
                    self.options,
                )
                .await
        })
    }
}

impl<'a> From<ClientSetLookupBuilder<'a>> for Operation {
    fn from(builder: ClientSetLookupBuilder<'a>) -> Self {
        Operation::SetLookup {
            entity: builder.entity,
            id: builder.id,
            nav_property: builder.nav_property,
            target_entity: builder.target_entity,
            target_id: builder.target_id,
            options: builder.options,
        }
    }
}

/// Builder for clear_lookup operations bound to a client.
pub struct ClientClearLookupBuilder<'a> {
    client: &'a DataverseClient,
    entity: Entity,
    id: Uuid,
    nav_property: String,
    options: OperationOptions,
}

impl<'a> ClientClearLookupBuilder<'a> {
    /// Skip custom plugin execution.
    pub fn bypass_plugins(mut self) -> Self {
        self.options.bypass_plugins = true;
        self
    }

    /// Skip Power Automate flows.
    pub fn bypass_flows(mut self) -> Self {
        self.options.bypass_flows = true;
        self
    }
}

impl<'a> std::future::IntoFuture for ClientClearLookupBuilder<'a> {
    type Output = Result<(), Error>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move {
            self.client
                .execute_clear_lookup(self.entity, self.id, &self.nav_property, self.options)
                .await
        })
    }
}

impl<'a> From<ClientClearLookupBuilder<'a>> for Operation {
    fn from(builder: ClientClearLookupBuilder<'a>) -> Self {
        Operation::ClearLookup {
            entity: builder.entity,
            id: builder.id,
            nav_property: builder.nav_property,
            options: builder.options,
        }
    }
}
