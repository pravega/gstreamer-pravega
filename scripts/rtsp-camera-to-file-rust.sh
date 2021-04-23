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
LOG_FILE=/mnt/data/logs/rtsp-camera-to-file-rust.log
# log level can be INFO or LOG (verbose)
export GST_DEBUG="rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,rtpsource:LOG,rtph264depay:LOG,\
h264parse:LOG,mpegtsmux:LOG,mpegtsbase:LOG,mpegtspacketizer:LOG,filesink:LOG,basesink:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
RTSP_URL="rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0"
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/rtsp-camera-to-file-rust
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --bin rtsp-camera-to-file -- --location "${RTSP_URL}" \
  $* \
  |& rotatelogs -L ${LOG_FILE} -p ${ROOT_DIR}/scripts/rotatelogs-compress.sh ${LOG_FILE} 1G
popd
