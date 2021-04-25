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
LOG_FILE=/mnt/data/logs/rtsp-camera-to-pravega.log
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:LOG,basesink:INFO,rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,identity:LOG
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtsp1}
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
CAMERA_PORT=${CAMERA_PORT:-554}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}:${CAMERA_PORT}/cam/realmonitor?channel=1&subtype=0" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  rtcp-sync-send-time=false \
! identity name=identity-from-rtspsrc silent=false \
! rtph264depay \
! h264parse \
! "video/x-h264,alignment=au" \
! mpegtsmux \
! identity name=from-mpegtsmux silent=false \
! queue max-size-buffers=0 max-size-bytes=10485760 max-size-time=0 silent=true leaky=downstream \
! identity name=from-queue silent=false \
! pravegasink \
  stream=examples/${PRAVEGA_STREAM} \
  controller=127.0.0.1:9090 \
  timestamp-mode=ntp \
  sync=false \
$* |& rotatelogs -L ${LOG_FILE} -p ${ROOT_DIR}/scripts/rotatelogs-compress.sh ${LOG_FILE} 1G
