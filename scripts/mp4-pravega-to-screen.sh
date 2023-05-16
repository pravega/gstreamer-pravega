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
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
export PRAVEGA_VIDEO_LOG=debug
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-mp4-1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}


gst-launch-1.0 \
-v \
pravegasrc \
  allow-create-scope=${ALLOW_CREATE_SCOPE} \
  buffer-size=1024 \
  controller=${PRAVEGA_CONTROLLER_URI} \
  keycloak-file=\"${KEYCLOAK_SERVICE_ACCOUNT_FILE}\" \
  stream=${PRAVEGA_SCOPE}/${PRAVEGA_STREAM} \
  $* \
! qtdemux name=qtdemux \
! h264parse name=h264parse \
! video/x-h264,alignment=au \
! avdec_h264 \
! videoconvert \
! autovideosink sync=true