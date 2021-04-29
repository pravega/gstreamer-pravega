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
mod pravega_service;
mod utils;

use pravega_client_config::ClientConfig;
use pravega_client_config::ClientConfigBuilder;
use std::process::Command;
use std::{thread, time};
use tracing::{error, info, info_span, warn};

#[macro_use]
extern crate derive_new;

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

#[derive(new, Clone, Debug)]
pub struct TestConfig {
    pub client_config: ClientConfig,
    pub scope: String,
    pub test_id: String,
}

#[cfg(test)]
mod test {
    use std::env;
    use tracing::{error, info, info_span, warn};
    use tracing_subscriber::fmt::format::FmtSpan;
    use crate::pravega_service::{PravegaService, PravegaStandaloneService, PravegaStandaloneServiceConfig};
    use super::*;

    #[test]
    fn integration_test() {
        // Valid log levels: error,warn,info,debug,trace
        let filter = std::env::var("GST_PRAVEGA_INTEGRATION_TEST_LOG")
            .unwrap_or_else(|_| "gstreamer_pravega_integration_test=debug,warn".to_owned());
        if !filter.is_empty() {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_span_events(FmtSpan::CLOSE)
                .try_init()
                .unwrap();
        }
        info!("Running gstreamer-pravega integration tests");
        let config = PravegaStandaloneServiceConfig::new(false, false, false);
        run_tests(config);
    }

    fn run_tests(config: PravegaStandaloneServiceConfig) {
        let test_id = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
        info!("test_id={}", test_id);

        // Start Pravega standalone.
        let mut pravega = PravegaStandaloneService::start(config.clone());
        wait_for_standalone_with_timeout(true, 30);

        // Configure Pravega client.
        if config.auth {
            env::set_var("pravega_client_auth_method", "Basic");
            env::set_var("pravega_client_auth_username", "admin");
            env::set_var("pravega_client_auth_password", "1111_aaaa");
        }
        let controller_uri = pravega.get_controller_uri();
        let client_config = ClientConfigBuilder::default()
            .controller_uri(controller_uri)
            .is_auth_enabled(config.auth)
            .is_tls_enabled(config.tls)
            .build()
            .unwrap();
        let scope = "examples".to_owned();
        let test_config = TestConfig::new(
            client_config,
            scope,
            test_id,
        );
        info!("test_config={:?}", test_config);

        let tests = vec![
            (gst_plugin_pravega_tests::test_raw_video, "test_raw_video"),
        ];

        for test in tests.iter() {
            let span = info_span!("test", test = test.1, auth = config.auth, tls = config.tls);
            span.in_scope(|| {
                info!("Running {}", test.1);
                test.0(test_config.clone());
            });
        }

        // Shut down Pravega standalone.
        pravega.stop().unwrap();
        wait_for_standalone_with_timeout(false, 30);
    }
}
