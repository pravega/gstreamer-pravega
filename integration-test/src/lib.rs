//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

mod gst_plugin_pravega_tests;
mod pravegasrc_tests;
mod pravega_service;
mod rtsp_camera_simulator;
mod rtsp_tests;
mod utils;

use lazy_static::lazy_static;
use pravega_client_config::ClientConfig;
use pravega_client_config::ClientConfigBuilder;
use std::process::Command;
use std::sync::Mutex;
use std::{thread, time};
#[allow(unused_imports)]
use tracing::{error, info, info_span, warn};
#[cfg(test)]
use tracing_subscriber::fmt::format::FmtSpan;
use crate::pravega_service::{PravegaService, PravegaStandaloneService, PravegaStandaloneServiceConfig};

#[macro_use]
extern crate derive_new;

#[derive(Clone, Debug)]
pub struct TestConfig {
    pub client_config: ClientConfig,
    pub scope: String,
    pub test_id: String,
}

impl TestConfig {
    pub fn pravega_plugin_properties(&self, stream_name: &str) -> String {
        format!("controller={controller_uri} stream={scope}/{stream_name}",
            controller_uri = self.client_config.clone().controller_uri.0,
            scope = self.scope,
            stream_name = stream_name,
        )
    }
}

/// Get test configuration for all integration tests.
/// This will start a Pravega standalone server by default.
pub fn get_test_config() -> TestConfig {
    TestConfig {
        client_config: get_client_config(),
        scope: get_scope(),
        test_id: get_test_id(),
    }
}

fn get_test_id() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string()
}

fn get_scope() -> String {
    "test".to_owned()
}

/// Get the Pravega ClientConfig for all integration tests.
/// If the environment variable PRAVEGA_CONTROLLER_URI is set, it will be used.
/// Otherwise, it will start a Pravega standalone server.
/// The Pravega standalone server will be stopped when the process terminates by shutdown_pravega_standalone().
fn get_client_config() -> ClientConfig {
    let controller_uri = match std::env::var("PRAVEGA_CONTROLLER_URI") {
        Ok(controller_uri) => {
            info!("Using external Pravega server with controller {}", controller_uri);
            controller_uri
        },
        Err(_) => {
            let mut pravega_service_opt = PRAVEGA_SERVICE.lock().unwrap();
            // Start Pravega standalone if we haven't started it yet.
            match &mut *pravega_service_opt {
                Some(_) => (),
                None => {
                    let config = PravegaStandaloneServiceConfig::new(false, false, false);
                    let pravega_service = Some(PravegaStandaloneService::start(config));
                    wait_for_standalone_with_timeout(true, 30);
                    *pravega_service_opt = pravega_service;
                },
            };
            let pravega_service = match &mut *pravega_service_opt {
                Some(pravega_service) => pravega_service,
                None => unreachable!(),
                };
            pravega_service.get_controller_uri()
        }
    };
    let client_config = ClientConfigBuilder::default()
        .controller_uri(controller_uri)
        .is_auth_enabled(false)
        .is_tls_enabled(false)
        .build()
        .unwrap();
    info!("Pravega client config: {:?}", client_config);
    client_config
}

fn wait_for_standalone_with_timeout(expected_status: bool, timeout_second: i32) {
    for _i in 0..timeout_second {
        if expected_status == check_standalone_status() {
            if expected_status {
                info!("Pravega is running.");
            }
            return;
        }
        thread::sleep(time::Duration::from_secs(1));
    }
    panic!(
        "timeout {} exceeded, Pravega standalone is in status {} while expected {}",
        timeout_second, !expected_status, expected_status
    );
}

fn check_standalone_status() -> bool {
    let output = Command::new("sh")
        .arg("-c")
        .arg("netstat -ltn 2> /dev/null | grep 9090 || ss -ltn 2> /dev/null | grep 9090")
        .output()
        .expect("failed to execute process");
    // if length is not zero, controller is listening on port 9090
    !output.stdout.is_empty()
}

lazy_static! {
    static ref PRAVEGA_SERVICE: Mutex<Option<PravegaStandaloneService>> = Mutex::new(None);
}

/// Initialize tracing in the module constructor so that tracing is available in all tests.
/// Tracing can be customized by setting the environment variable GST_PRAVEGA_INTEGRATION_TEST_LOG.
#[cfg(test)]
#[ctor::ctor]
fn init() {
    let filter = std::env::var("GST_PRAVEGA_INTEGRATION_TEST_LOG")
        .unwrap_or_else(|_| "gstreamer_pravega_integration_test=debug,pravega_video=debug,warn".to_owned());
    if !filter.is_empty() {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_span_events(FmtSpan::CLOSE)
            .try_init()
            .unwrap();
    }
}

/// If Pravega standalone was started, it will be stopped when this process exits.
/// The shutdown function must not use println or tracing or a panic will occur.
#[cfg(test)]
#[ctor::dtor]
unsafe fn shutdown_pravega_standalone() {
    let mut pravega_service_opt = PRAVEGA_SERVICE.lock().unwrap();
    match &mut *pravega_service_opt {
        Some(pravega_service) => {
            pravega_service.stop().unwrap();
            wait_for_standalone_with_timeout(false, 30);
        },
        None => (),
    };
}
