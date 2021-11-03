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

# Run RTSP Camera Simulator with RTSP over TLS.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
# log level can be INFO or LOG (verbose)
# export GST_DEBUG="INFO"
# export RUST_LOG=debug
# export RUST_BACKTRACE=1
export CAMERA_USER=admin
export CAMERA_PASSWORD=password
export TLS_CERT_FILE=${ROOT_DIR}/tls/localhost.crt
export TLS_KEY_FILE=${ROOT_DIR}/tls/localhost.key
export TZ=UTC
pushd ${ROOT_DIR}/apps
cargo run --bin rtsp-camera-simulator -- $* \
|& tee /tmp/rtsp-camera-simulator.log
popd
