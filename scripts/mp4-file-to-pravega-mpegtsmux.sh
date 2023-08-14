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
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO
export RUST_BACKTRACE=1
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-mp4-1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-127.0.0.1:9090}
VIDEO_FILE=${VIDEO_FILE:-/file/path/name.mp4}
FPS=25
KEY_FRAME_INTERVAL=$((1*$FPS))

gst-launch-1.0 \
-v \
filesrc location=${VIDEO_FILE} \
! decodebin \
! videoconvert \
! x264enc tune=zerolatency key-int-max=30 speed-preset=medium \
! h264parse \
! video/x-h264,alignment=au \
! mpegtsmux \
! pravegasink \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  controller=${PRAVEGA_CONTROLLER} \
  keycloak-file=\"${KEYCLOAK_SERVICE_ACCOUNT_FILE}\" \
  seal=false sync=false timestamp-mode=realtime-clock

