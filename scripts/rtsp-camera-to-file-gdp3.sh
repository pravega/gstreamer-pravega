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
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_ADDRESS=${CAMERA_ADDRESS:-192.168.1.102}
OUTPUT_FILE=${HOME}/rtsp3.gdp
export GST_DEBUG="FIXME,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO"
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/$(basename "${0}" .sh)
rm -f ${GST_DEBUG_DUMP_DOT_DIR}/*
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_ADDRESS}/cam/realmonitor?channel=1&subtype=0" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  max-ts-offset=1000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  onvif-mode=true \
! application/x-rtp,media=video \
! identity silent=false eos-after=10000 \
! gdppay \
! filesink location=${OUTPUT_FILE} \
  sync=false \
$* |& tee ${LOG_FILE}

${ROOT_DIR}/scripts/dot-to-png.sh /tmp/gst-dot/$(basename "${0}" .sh)/*.dot
