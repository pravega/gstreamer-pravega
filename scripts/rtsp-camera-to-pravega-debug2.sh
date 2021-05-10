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
LOG_FILE=/tmp/rtsp-camera-to-pravega.log
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:LOG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO,identity:INFO
export RUST_BACKTRACE=1
CAMERA_IP=${CAMERA_IP:-127.0.0.1}
CAMERA_PASSWORD=${CAMERA_USER:-password}
CAMERA_PATH=${CAMERA_PATH:-"/cam/realmonitor?target_rate_kilobytes_per_sec=100"}
CAMERA_PORT=${CAMERA_PORT:-8554}
CAMERA_USER=${CAMERA_USER:-user}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtsp2}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD}@${CAMERA_IP}:${CAMERA_PORT}${CAMERA_PATH}" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  rtcp-sync-send-time=false \
! identity name=identity-from-rtspsrc silent=true \
! rtph264depay \
! h264parse \
! "video/x-h264,alignment=au" \
! mpegtsmux \
! identity name=from-mpegtsmux silent=true \
! queue max-size-buffers=0 max-size-bytes=10000 max-size-time=0 silent=false leaky=downstream \
! identity name=from-queue silent=true \
! pravegasink \
  stream=examples/${PRAVEGA_STREAM} \
  controller=127.0.0.1:9090 \
  timestamp-mode=ntp \
  sync=false \
$* |& tee ${LOG_FILE}
