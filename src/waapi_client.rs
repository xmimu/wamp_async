use crate::{try_into_kwargs, try_into_wamp_dict, Client, ClientConfig, ClientRole, SerializerType, WampError};
use serde_json::{json, Value};
use std::error::Error as StdError;
use std::fmt;

/// Configuration builder for WAAPI client
pub struct WaapiClientBuilder {
    host: String,
    port: u16,
    realm: String,
    ssl_verify: bool,
}

impl WaapiClientBuilder {
    /// Create a new WAAPI client builder with default settings
    /// 
    /// Default values:
    /// - host: "localhost"
    /// - port: 8080
    /// - realm: "realm1"
    /// - ssl_verify: false
    pub fn new() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 8080,
            realm: "realm1".to_string(),
            ssl_verify: false,
        }
    }

    /// Set the WAAPI server host
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the WAAPI server port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the realm name
    pub fn realm(mut self, realm: impl Into<String>) -> Self {
        self.realm = realm.into();
        self
    }

    /// Set whether to verify SSL certificates
    pub fn ssl_verify(mut self, verify: bool) -> Self {
        self.ssl_verify = verify;
        self
    }

    /// Connect to the WAAPI server and return a ready-to-use client
    /// 
    /// This method will:
    /// 1. Establish WebSocket connection
    /// 2. Start the event loop automatically
    /// 3. Join the specified realm
    /// 
    /// # Errors
    /// 
    /// Returns an error if connection fails or realm joining fails
    pub async fn connect(self) -> Result<WaapiClient, WaapiError> {
        // Build WebSocket URL
        let url = format!("ws://{}:{}/waapi", self.host, self.port);

        // Create client configuration
        let config = ClientConfig::default()
            .set_ssl_verify(self.ssl_verify)
            .set_roles(vec![ClientRole::Caller])
            .set_serializers(vec![SerializerType::Json]);

        // Connect to server
        let (mut client, (evt_loop, _rpc_evt_queue)) = Client::connect(&url, Some(config))
            .await
            .map_err(WaapiError::ConnectionFailed)?;

        // Spawn event loop automatically
        tokio::spawn(evt_loop);

        // Join realm automatically
        client
            .join_realm(&self.realm)
            .await
            .map_err(WaapiError::JoinRealmFailed)?;

        Ok(WaapiClient {
            client,
            realm: self.realm,
        })
    }
}

impl Default for WaapiClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A simplified WAAPI client that manages connection lifecycle automatically
pub struct WaapiClient {
    client: Client<'static>,
    realm: String,
}

impl WaapiClient {
    /// Create a new builder for configuring the WAAPI client
    pub fn builder() -> WaapiClientBuilder {
        WaapiClientBuilder::new()
    }

    /// Call a WAAPI RPC procedure
    /// 
    /// # Arguments
    /// 
    /// * `procedure` - The WAAPI procedure URI (e.g., "ak.wwise.core.object.get")
    /// * `kwargs` - Optional keyword arguments as JSON object
    /// * `options` - Optional call options as JSON object
    /// 
    /// # Returns
    /// 
    /// Returns the result as a `serde_json::Value`. WAAPI typically returns results
    /// in the `kwargs` format (JSON object), which is what this method returns.
    /// 
    /// # Examples
    /// 
    /// ```no_run
    /// use serde_json::json;
    /// use wamp_async::WaapiClient;
    /// 
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut client = WaapiClient::builder()
    ///     .host("localhost")
    ///     .port(8080)
    ///     .connect()
    ///     .await?;
    /// 
    /// let result = client.call(
    ///     "ak.wwise.core.object.get",
    ///     Some(json!({"from": {"ofType": ["Event"]}})),
    ///     Some(json!({"return": ["name", "id"]}))
    /// ).await?;
    /// 
    /// println!("Result: {}", result);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call(
        &mut self,
        procedure: &str,
        kwargs: Option<Value>,
        options: Option<Value>,
    ) -> Result<Value, WaapiError> {
        // WAAPI doesn't use positional arguments, only kwargs
        let wamp_args = None;

        let wamp_kwargs = kwargs
            .map(try_into_kwargs)
            .transpose()
            .map_err(|e| WaapiError::ArgumentConversionFailed(Box::new(e)))?;

        let wamp_options = options
            .map(try_into_wamp_dict)
            .transpose()
            .map_err(|e| WaapiError::OptionsConversionFailed(Box::new(e)))?;

        // Call the RPC procedure
        let (res_args, res_kwargs) = self
            .client
            .call(procedure, wamp_args, wamp_kwargs, wamp_options)
            .await
            .map_err(WaapiError::CallFailed)?;

        // Convert result to JSON Value
        // WAAPI typically returns kwargs (object), so prioritize that
        let result = match (res_args, res_kwargs) {
            (_, Some(kwargs)) => {
                // Convert Map to Value
                json!(kwargs)
            }
            (Some(args), None) => {
                // Return args as array
                Value::Array(args)
            }
            (None, None) => {
                // No result, return empty object
                json!({})
            }
        };

        Ok(result)
    }

    /// Check if the client is connected
    pub fn is_connected(&mut self) -> bool {
        self.client.is_connected()
    }

    /// Get the realm name this client is connected to
    pub fn realm(&self) -> &str {
        &self.realm
    }

    /// Close the connection gracefully
    /// 
    /// This will leave the realm and disconnect from the server.
    pub async fn close(mut self) -> Result<(), WaapiError> {
        self.client
            .leave_realm()
            .await
            .map_err(WaapiError::LeaveRealmFailed)?;

        self.client.disconnect().await;
        Ok(())
    }
}

/// Error type for WAAPI client operations
#[derive(Debug)]
pub enum WaapiError {
    /// Failed to establish connection to WAAPI server
    ConnectionFailed(WampError),
    /// Failed to join the specified realm
    JoinRealmFailed(WampError),
    /// Failed to leave the realm
    LeaveRealmFailed(WampError),
    /// RPC call failed
    CallFailed(WampError),
    /// Failed to convert arguments to WAMP format
    ArgumentConversionFailed(Box<dyn StdError + Send + Sync>),
    /// Failed to convert options to WAMP format
    OptionsConversionFailed(Box<dyn StdError + Send + Sync>),
}

impl fmt::Display for WaapiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WaapiError::ConnectionFailed(e) => write!(f, "Failed to connect to WAAPI server: {}", e),
            WaapiError::JoinRealmFailed(e) => write!(f, "Failed to join realm: {}", e),
            WaapiError::LeaveRealmFailed(e) => write!(f, "Failed to leave realm: {}", e),
            WaapiError::CallFailed(e) => write!(f, "RPC call failed: {}", e),
            WaapiError::ArgumentConversionFailed(e) => write!(f, "Failed to convert arguments: {}", e),
            WaapiError::OptionsConversionFailed(e) => write!(f, "Failed to convert options: {}", e),
        }
    }
}

impl StdError for WaapiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            WaapiError::ConnectionFailed(e) 
            | WaapiError::JoinRealmFailed(e) 
            | WaapiError::LeaveRealmFailed(e)
            | WaapiError::CallFailed(e) => Some(e),
            _ => None,
        }
    }
}
