//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// Pravega utility functions.

use std::net::{SocketAddr, AddrParseError};
use std::time::{Duration, UNIX_EPOCH};
use futures::executor;

use pravega_client::byte::ByteReader;
use pravega_client_config::{ClientConfig, ClientConfigBuilder};
use pravega_client_config::credentials::Credentials;

/// A trait that allows retrieval of the current head of a Pravega byte stream.
/// The default implementation returns 0 to indicate that no data has been truncated.
pub trait CurrentHead {
    fn current_head(&self) -> std::io::Result<u64> {
        Ok(0)
    }
}

impl CurrentHead for ByteReader {
    fn current_head(&self) -> std::io::Result<u64> {
        executor::block_on(self.current_head())
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

pub fn create_client_config(controller: String, keycloak_file: Option<String>) -> Result<ClientConfig, String> {
    let (is_auth_enabled, credential) = match keycloak_file {
        Some(keycloak_file) => {
            if keycloak_file.is_empty() {
                (false, Credentials::basic("".into(), "".into()))
            } else {
                (true, Credentials::keycloak(&keycloak_file[..]))
            }
        },
        None => (false, Credentials::basic("".into(), "".into()))
    };
    ClientConfigBuilder::default()
        .controller_uri(controller)
        .max_connections_in_pool(0u32)
        .is_auth_enabled(is_auth_enabled)
        .credentials(credential)
        .build()
}
