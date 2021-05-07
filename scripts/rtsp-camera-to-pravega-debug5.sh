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
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=mpegtsbase:LOG,mpegtspacketizer:LOG,pravegasink:LOG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO,identity:INFO,INFO
export RUST_BACKTRACE=1
export GST_DEBUG_DUMP_DOT_DIR="/tmp/gst-dot/$(basename "${0}" .sh)"
rm -rf "${GST_DEBUG_DUMP_DOT_DIR}"
mkdir -p "${GST_DEBUG_DUMP_DOT_DIR}"

TARGET_RATE_KB_PER_SEC=100
BITRATE_KILOBITS_PER_SEC=$(( ${TARGET_RATE_KB_PER_SEC} * 8 ))
CAMERA_IP=${CAMERA_IP:-127.0.0.1}
CAMERA_PASSWORD=${CAMERA_USER:-password}
CAMERA_PATH=${CAMERA_PATH:-"/cam/realmonitor?width=640&height=480&fps=30&show_time=false&target_rate_kilobytes_per_sec=${TARGET_RATE_KB_PER_SEC}"}
CAMERA_PORT=${CAMERA_PORT:-8554}
CAMERA_USER=${CAMERA_USER:-user}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtsp6}

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
! rtph264depay \
! h264parse \
! "video/x-h264,alignment=au,stream-format=byte-stream" \
! identity eos-after=300 silent=true \
! mpegtsmux \
! filesink \
  location=${HOME}/test.ts \
$* |& tee ${LOG_FILE}

${ROOT_DIR}/scripts/dot-to-png.sh ${GST_DEBUG_DUMP_DOT_DIR}/*.dot
xdg-open ${GST_DEBUG_DUMP_DOT_DIR}

ls -lh ${HOME}/test.ts
