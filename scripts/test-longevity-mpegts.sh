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
LOG_FILE="/tmp/$(basename "${0}" .sh).log"

PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-tcp://127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-mpegts1}

export RUST_BACKTRACE=1

pushd ${ROOT_DIR}/integration-test

cargo run --bin longevity-test -- \
--stream ${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
--controller ${PRAVEGA_CONTROLLER_URI} \
--container-format mpegts \
|& tee ${LOG_FILE}
