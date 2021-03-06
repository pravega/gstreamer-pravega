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

export PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-tcp://127.0.0.1:9090}
export PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
export PRAVEGA_STREAM=${PRAVEGA_STREAM:-mp43}
# export START_UTC=2021-05-26T17:07:08Z
# export END_UTC=2021-05-26T17:07:13Z
# export RUST_LOG=longevity_test=debug,warn
export RUST_BACKTRACE=1
LOG_FILE="/tmp/$(basename "${0}" .sh)-${PRAVEGA_STREAM}.log"

pushd ${ROOT_DIR}/integration-test

cargo run --bin longevity-test -- $* |& tee ${LOG_FILE}
