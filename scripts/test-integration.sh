#!/usr/bin/env bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

# If PRAVEGA_CONTROLLER_URI is not set, then Pravega standalone will be started and stopped by the integration test.
# For example: export PRAVEGA_CONTROLLER_URI=127.0.0.1:9090

# If RTSP_URL is set, it will be used for all RTSP tests. This allows for real cameras to be used.
# Otherwise, the integration test will run an in-process RTSP camera simulator that is appropriate for each test.
# See rtsp-env-sample.sh for an example.

# Multiple test threads can be used but troubleshooting is easier with just 1 thread.
# When increasing this, we recommend using a Pravega server started with ../pravega-docker/up.sh
# to better handle the high load.
TEST_THREADS=${TEST_THREADS:-1}

pushd ${ROOT_DIR}/integration-test
export RUST_BACKTRACE=0
export JUNIT_OUTPUT=${JUNIT_OUTPUT:-0}

# Build tests then print list of test names.
# This will ignore any tests with names containing "ignore".
cargo test --release --locked $* -- --skip ignore --list \
|& tee /tmp/integration-test.log

if [[ "${BUILD_PROD}" != "0" ]]; then
    # Run tests.
    cargo test --release --locked $* -- --skip ignore --nocapture --test-threads=${TEST_THREADS} \
    -Z unstable-options --format json | cargo2junit | tee junit.xml
else
    # Run tests.
    cargo test --release --locked $* -- --skip ignore --nocapture --test-threads=${TEST_THREADS} \
    |& tee -a /tmp/integration-test.log
fi
