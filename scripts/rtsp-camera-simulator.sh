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
# export GST_DEBUG="x264enc:INFO"
# export RUST_LOG=debug
# export RUST_BACKTRACE=1
export TZ=UTC
pushd ${ROOT_DIR}/apps
cargo run --bin rtsp-camera-simulator -- $* \
|& tee /tmp/rtsp-camera-simulator.log
popd
