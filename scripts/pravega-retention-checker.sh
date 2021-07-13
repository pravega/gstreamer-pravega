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
pushd ${ROOT_DIR}/apps
cargo build
popd

export RUST_LOG=info,pravega_video:debug
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-camera8}
CHECK_PERIOD=${CHECK_PERIOD:-60}

pushd ${ROOT_DIR}/apps
cargo run --bin pravega_retention_checker -- \
--controller ${PRAVEGA_CONTROLLER_URI} \
--scope ${PRAVEGA_SCOPE} \
--stream ${PRAVEGA_STREAM} \
--keycloak-file "${KEYCLOAK_SERVICE_ACCOUNT_FILE}" \
--check-period $CHECK_PERIOD
$* \
|& tee /tmp/pravega-retention-checker.log
popd
