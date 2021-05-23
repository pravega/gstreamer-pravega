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
# log level can be INFO or LOG (verbose)
export RUST_LOG=info
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
pushd ${ROOT_DIR}/pravega-video-server
if [[ ! -z "${KEYCLOAK_SERVICE_ACCOUNT_FILE}" ]]; then
    AUTH_OPTS="--keycloak-file ${KEYCLOAK_SERVICE_ACCOUNT_FILE}"
else
    AUTH_OPTS=""
fi
cargo run -- \
--controller ${PRAVEGA_CONTROLLER_URI} \
${AUTH_OPTS} \
$* \
|& tee /tmp/pravega-video-server.log
popd
