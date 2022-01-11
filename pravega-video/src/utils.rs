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
use std::io::{Read, Seek, SeekFrom};

use pravega_client::byte::ByteReader;
use pravega_client_config::{ClientConfig, ClientConfigBuilder};
use pravega_client_config::credentials::Credentials;

use tokio::runtime::Handle;

pub const DEFAULT_PRAVEGA_CONTROLLER_URI: &str = "tcp://127.0.0.1:9090";
pub const ENV_PRAVEGA_CONTROLLER_URI: &str = "PRAVEGA_CONTROLLER_URI";
pub const ENV_KEYCLOAK_SERVICE_ACCOUNT_FILE: &str = "KEYCLOAK_SERVICE_ACCOUNT_FILE";

pub struct SyncByteReader {
    byte_reader: ByteReader,
    runtime_handle: Handle,
}

impl SyncByteReader {
    pub fn new(byte_reader: ByteReader, runtime_handle: Handle) -> Self {
        Self {
            byte_reader,
            runtime_handle,
        }
    }
}

impl Read for SyncByteReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.runtime_handle.block_on(self.byte_reader.read(buf))
    }
}

impl Seek for SyncByteReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.runtime_handle.block_on(self.byte_reader.seek(pos))
    }
}

/// A trait that allows retrieval of the current head of a Pravega byte stream.
/// The default implementation returns 0 to indicate that no data has been truncated.
pub trait CurrentHead {
    fn current_head(&self) -> std::io::Result<u64> {
        Ok(0)
    }
}

impl CurrentHead for SyncByteReader {
    fn current_head(&self) -> std::io::Result<u64> {
        self.runtime_handle.block_on(self.byte_reader.current_head())
    }
}

impl<T> CurrentHead for std::io::Cursor<T> {}

pub fn parse_controller_uri(controller: String) -> Result<SocketAddr, AddrParseError> {
    controller.parse::<SocketAddr>()
}

/// See [event_serde::EventWriter] for a description of timestamp.
pub fn format_pravega_timestamp(timestamp: u64) -> String {
    let system_time = UNIX_EPOCH + Duration::from_micros(timestamp);
    let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
    let formatted_time = datetime.format("%Y-%m-%d %T.%6f");
    formatted_time.to_string()
}

/// Return the Pravega controller URI from the environment if it exists or the URI for a local server.
pub fn default_pravega_controller_uri() -> Option<String> {
    match std::env::var(ENV_PRAVEGA_CONTROLLER_URI) {
        Ok(value) => if value.is_empty() {
            Some(DEFAULT_PRAVEGA_CONTROLLER_URI.to_owned())
        } else {
            Some(value)
        },
        Err(_) => Some(DEFAULT_PRAVEGA_CONTROLLER_URI.to_owned()),
    }
}

/// Return the keycloak file name from the environment if it exists or None.
pub fn default_keycloak_file() -> Option<String> {
    match std::env::var(ENV_KEYCLOAK_SERVICE_ACCOUNT_FILE) {
        Ok(value) => if value.is_empty() {
            None
        } else {
            Some(value)
        },
        Err(_) => None,
    }
}

/// Return a Pravega ClientConfig.
///
/// * `controller` - Pravega controller URI
/// * `keycloak_file` - name of the file containing the Keycloak JSON, or None to disable authentication
pub fn create_client_config(controller: String, keycloak_file: Option<String>) -> Result<ClientConfig, String> {
    let (is_auth_enabled, credential) = match keycloak_file {
        Some(keycloak_file) => {
            if keycloak_file.is_empty() {
                (false, Credentials::basic("".into(), "".into()))
            } else {
                (true, Credentials::keycloak(&keycloak_file[..], false))
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

pub fn get_video_tags() -> Option<Vec<String>> {
    Some(vec![get_video_tag_query()])
}

pub fn get_video_tag_query() -> String {
    "video".to_string()
}
