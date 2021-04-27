//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

mod byte_stream_tests;
mod pravega_service;
mod utils;

use crate::pravega_service::{PravegaService, PravegaStandaloneService};
use lazy_static::*;
use std::process::Command;
use std::{thread, time};
use tracing::{error, info, info_span, warn};

#[macro_use]
extern crate derive_new;

fn wait_for_standalone_with_timeout(expected_status: bool, timeout_second: i32) {
    for _i in 0..timeout_second {
        if expected_status == check_standalone_status() {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::pravega_service::PravegaStandaloneServiceConfig;
    use pravega_client::trace;
    use std::env;
    use std::net::SocketAddr;

    #[test]
    fn integration_test() {
        trace::init();
        info!("Running integration test");
        // let config = PravegaStandaloneServiceConfig::new(false, true, true);
        // run_tests(config);
        let config = PravegaStandaloneServiceConfig::new(false, false, false);
        run_tests(config);
    }

    fn run_tests(config: PravegaStandaloneServiceConfig) {
        let mut pravega = PravegaStandaloneService::start(config.clone());
        wait_for_standalone_with_timeout(true, 30);
        if config.auth {
            env::set_var("pravega_client_auth_method", "Basic");
            env::set_var("pravega_client_auth_username", "admin");
            env::set_var("pravega_client_auth_password", "1111_aaaa");
        }
        let span = info_span!("byte stream test", auth = config.auth, tls = config.tls);
        span.in_scope(|| {
            info!("Running byte stream test");
            byte_stream_tests::test_byte_stream(config.clone());
        });
        // Shut down Pravega standalone
        pravega.stop().unwrap();
        wait_for_standalone_with_timeout(false, 30);
    }
}
