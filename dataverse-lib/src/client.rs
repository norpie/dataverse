//! Main DataverseClient

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::auth::TokenProvider;
use crate::error::ApiError;
use crate::error::Error;

/// The main client for interacting with the Dataverse Web API.
///
/// This client is cheap to clone (uses `Arc` internally) and can be shared
/// across threads safely.
///
/// # Example
///
/// ```ignore
/// use dataverse_lib::{DataverseClient, auth::StaticTokenProvider};
///
/// let provider = StaticTokenProvider::new("my-token");
/// let client = DataverseClient::builder()
///     .url("https://org.crm.dynamics.com")
///     .token_provider(provider)
///     .build();
///
/// client.connect().await?;
/// ```
#[derive(Clone)]
pub struct DataverseClient {
    inner: Arc<DataverseClientInner>,
}

struct DataverseClientInner {
    base_url: String,
    api_version: String,
    token_provider: Arc<dyn TokenProvider>,
    http_client: Client,
    timeout: Option<Duration>,
}

impl DataverseClient {
    /// Creates a new builder for constructing a client.
    pub fn builder() -> DataverseClientBuilder<Missing, Missing> {
        DataverseClientBuilder::new()
    }

    /// Validates connectivity to the Dataverse environment.
    ///
    /// Makes a `WhoAmI` request to verify the connection and credentials are valid.
    pub async fn connect(&self) -> Result<WhoAmIResponse, Error> {
        let url = format!(
            "{}/api/data/{}/WhoAmI",
            self.inner.base_url.trim_end_matches('/'),
            self.inner.api_version
        );

        let token = self
            .inner
            .token_provider
            .get_token(&self.inner.base_url)
            .await?;

        let mut request = self.inner.http_client.get(&url).bearer_auth(&token.access_token);

        if let Some(timeout) = self.inner.timeout {
            request = request.timeout(timeout);
        }

        let response = request.send().await.map_err(ApiError::from)?;

        if response.status().is_success() {
            let who_am_i: WhoAmIResponse = response.json().await.map_err(ApiError::from)?;
            Ok(who_am_i)
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err(Error::Api(ApiError::Http {
                status,
                message: body,
                code: None,
                inner: None,
            }))
        }
    }

    /// Returns the base URL of the Dataverse environment.
    pub fn base_url(&self) -> &str {
        &self.inner.base_url
    }

    /// Returns the API version being used.
    pub fn api_version(&self) -> &str {
        &self.inner.api_version
    }
}

/// Response from the WhoAmI request.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct WhoAmIResponse {
    /// The ID of the business unit.
    pub business_unit_id: uuid::Uuid,
    /// The ID of the current user.
    pub user_id: uuid::Uuid,
    /// The ID of the organization.
    pub organization_id: uuid::Uuid,
}

// =============================================================================
// Typestate Builder
// =============================================================================

/// Marker type for missing required builder fields.
pub struct Missing;

/// Marker type for set builder fields.
pub struct Set<T>(T);

/// Builder for constructing a [`DataverseClient`].
///
/// Uses the typestate pattern to ensure required fields are set at compile time.
///
/// # Required Fields
///
/// - `url` - The Dataverse environment URL
/// - `token_provider` - A [`TokenProvider`] implementation
///
/// # Example
///
/// ```ignore
/// let client = DataverseClient::builder()
///     .url("https://org.crm.dynamics.com")
///     .token_provider(my_provider)
///     .api_version("v9.2")
///     .timeout(Duration::from_secs(30))
///     .build();
/// ```
pub struct DataverseClientBuilder<Url, Provider> {
    url: Url,
    token_provider: Provider,
    api_version: String,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    http_client: Option<Client>,
}

impl DataverseClientBuilder<Missing, Missing> {
    /// Creates a new builder with default settings.
    pub fn new() -> Self {
        Self {
            url: Missing,
            token_provider: Missing,
            api_version: "v9.2".to_string(),
            timeout: None,
            connect_timeout: None,
            http_client: None,
        }
    }
}

impl Default for DataverseClientBuilder<Missing, Missing> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> DataverseClientBuilder<Missing, P> {
    /// Sets the Dataverse environment URL.
    ///
    /// # Example
    ///
    /// ```ignore
    /// .url("https://org.crm.dynamics.com")
    /// ```
    pub fn url(self, url: impl Into<String>) -> DataverseClientBuilder<Set<String>, P> {
        DataverseClientBuilder {
            url: Set(url.into()),
            token_provider: self.token_provider,
            api_version: self.api_version,
            timeout: self.timeout,
            connect_timeout: self.connect_timeout,
            http_client: self.http_client,
        }
    }
}

impl<U> DataverseClientBuilder<U, Missing> {
    /// Sets the token provider for authentication.
    pub fn token_provider<T: TokenProvider + 'static>(
        self,
        provider: T,
    ) -> DataverseClientBuilder<U, Set<Arc<dyn TokenProvider>>> {
        DataverseClientBuilder {
            url: self.url,
            token_provider: Set(Arc::new(provider) as Arc<dyn TokenProvider>),
            api_version: self.api_version,
            timeout: self.timeout,
            connect_timeout: self.connect_timeout,
            http_client: self.http_client,
        }
    }
}

impl<U, P> DataverseClientBuilder<U, P> {
    /// Sets the API version to use.
    ///
    /// Defaults to `v9.2`.
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    /// Sets the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the connection timeout.
    ///
    /// This is applied when building the HTTP client.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Sets a custom HTTP client.
    ///
    /// If not set, a default client will be created.
    pub fn http_client(mut self, client: Client) -> Self {
        self.http_client = Some(client);
        self
    }
}

impl DataverseClientBuilder<Set<String>, Set<Arc<dyn TokenProvider>>> {
    /// Builds the [`DataverseClient`].
    ///
    /// This method is only available when both `url` and `token_provider` have been set.
    pub fn build(self) -> DataverseClient {
        let http_client = self.http_client.unwrap_or_else(|| {
            let mut builder = Client::builder();
            if let Some(timeout) = self.connect_timeout {
                builder = builder.connect_timeout(timeout);
            }
            builder.build().expect("Failed to build HTTP client")
        });

        DataverseClient {
            inner: Arc::new(DataverseClientInner {
                base_url: self.url.0,
                api_version: self.api_version,
                token_provider: self.token_provider.0,
                http_client,
                timeout: self.timeout,
            }),
        }
    }
}
