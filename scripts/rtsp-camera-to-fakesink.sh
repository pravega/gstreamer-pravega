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
export GST_DEBUG="INFO,rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,rtph264depay:LOG,h264parse:LOG,mpegtsmux:LOG,mpegtsbase:LOG,mpegtspacketizer:LOG,avdec_h264:LOG,videodecoder:LOG,filesink:LOG,basesink:INFO,identity:LOG"

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  "location=${RTSP_URL}" \
  buffer-mode=none \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  identity name=identity-from-rtspsrc silent=false \
! rtph264depay \
! h264parse \
! "video/x-h264,alignment=au" \
! identity name=identity-from-h264parse silent=false \
! avdec_h264 \
! identity name=identity-from-decodebin silent=false \
! fakesink \
  sync=false \
$* |& tee ${LOG_FILE}
