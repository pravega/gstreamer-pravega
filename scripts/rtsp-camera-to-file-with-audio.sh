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

CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
export GST_DEBUG="rtspsrc:LOG,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO,rtph264depay:INFO,h264parse:INFO,\
rtpmp4gdepay:LOG,aacparse:LOG,
mpegtsmux:LOG,mpegtsbase:LOG,mpegtspacketizer:LOG,filesink:LOG,basesink:INFO,identity:LOG"
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/rtsp-camera-to-file-with-audio
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc name=src \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
src. \
! rtph264depay \
! h264parse \
! "video/x-h264,alignment=au" \
! mux. \
src. \
! rtpmp4gdepay \
! aacparse \
! mux. \
mpegtsmux name=mux \
! filesink location=/mnt/data/tmp/rtsp-camera.ts \
  sync=false \
|& tee /mnt/data/logs/rtsp-camera-to-file-with-audio.log
