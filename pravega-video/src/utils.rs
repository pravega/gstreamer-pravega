// Pravega utility functions.

use std::net::{SocketAddr, AddrParseError};
use std::time::{Duration, UNIX_EPOCH};
use std::env;

use pravega_client::byte_stream::ByteStreamReader;
use pravega_client_config::ClientConfigBuilder;
use pravega_client_config::ClientConfig;

const AUTH_KEYCLOAK_PATH: &str = "pravega_client_auth_keycloak";

/// A trait that allows retrieval of the current head of a Pravega byte stream.
/// The default implementation returns 0 to indicate that no data has been truncated.
pub trait CurrentHead {
    fn current_head(&self) -> std::io::Result<u64> {
        Ok(0)
    }
}

impl CurrentHead for ByteStreamReader {
    fn current_head(&self) -> std::io::Result<u64> {
        self.current_head()
    }
}

impl<T> CurrentHead for std::io::Cursor<T> {}

pub fn parse_controller_uri(controller: String) -> Result<SocketAddr, AddrParseError> {
    controller.parse::<SocketAddr>()
}

// See [event_serde::EventWriter] for a description of timestamp.
pub fn format_pravega_timestamp(timestamp: u64) -> String {
    let system_time = UNIX_EPOCH + Duration::from_micros(timestamp);
    let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
    let formatted_time = datetime.format("%Y-%m-%d %T.%6f");
    formatted_time.to_string()
}

pub fn create_client_config(controller: String) -> Result<ClientConfig, String> {
    let is_tls_enabled = controller.starts_with("tls://");
    let controller_uri = 
        if controller.starts_with("tcp://") || controller.starts_with("tls://") {
            controller.chars().skip(6).collect()
        }
        else {
            controller
        };
    let is_auth_enabled = env::vars().any(|(k, _v)| k.starts_with(AUTH_KEYCLOAK_PATH));

    ClientConfigBuilder::default()
        .controller_uri(controller_uri)
        .is_auth_enabled(is_auth_enabled)
        .is_tls_enabled(is_tls_enabled)
        .build()
}