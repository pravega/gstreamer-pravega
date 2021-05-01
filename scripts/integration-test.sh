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
export PRAVEGA_CONTROLLER_URI=127.0.0.1:9090
pushd ${ROOT_DIR}/integration-test
export RUST_BACKTRACE=0
# Multiple test threads should work but troubleshooting is easier with just 1 thread.
TEST_THREADS=${TEST_THREADS:-1}
cargo test $* -- --nocapture --test-threads=${TEST_THREADS} \
|& tee /tmp/integration-test.log
