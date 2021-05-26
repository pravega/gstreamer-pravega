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

# Record video from an RTSP camera and write to Pravega.

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)

pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
popd

export ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
export BUFFER_SIZE_MB=${BUFFER_SIZE_MB:-50}
export CAMERA_ADDRESS=${CAMERA_ADDRESS:-127.0.0.1}
export CAMERA_PASSWORD=${CAMERA_PASSWORD:?Required environment variable not set}
export CAMERA_PATH=${CAMERA_PATH:-"/cam/realmonitor?target_rate_kilobytes_per_sec=25"}
export CAMERA_PORT=${CAMERA_PORT:-8554}
export CAMERA_USER=${CAMERA_USER:-user}
# log level can be INFO, DEBUG, or LOG (verbose)
#export GST_DEBUG=pravegasink:DEBUG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
export PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-tcp://127.0.0.1:9090}
export PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
export PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtsp1}
export RUST_BACKTRACE=1
LOG_FILE="/tmp/$(basename "${0}" .sh)-${PRAVEGA_STREAM}.log"

${ROOT_DIR}/python_apps/rtsp-camera-to-pravega.py \
$* |& tee ${LOG_FILE}

echo rtsp-camera-to-pravega-python.sh: END
