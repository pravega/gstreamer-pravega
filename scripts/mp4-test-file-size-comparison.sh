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

# Compare MP4 and MPEG TS file sizes.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
LOG_FILE="/tmp/$(basename "${0}" .sh).log"
export GST_DEBUG=qtmux:INFO,FIXME
FRAGMENT_DURATION_MS=100
TARGET_RATE_KB_PER_SEC=100
BITRATE_KILOBITS_PER_SEC=$(( ${TARGET_RATE_KB_PER_SEC} * 8 ))

CAMERA_ADDRESS=${CAMERA_ADDRESS:-127.0.0.1}
CAMERA_PASSWORD=${CAMERA_PASSWORD:-password}
CAMERA_PATH=${CAMERA_PATH:-"/cam/realmonitor?width=640&height=480&fps=30&show_time=false&target_rate_kilobytes_per_sec=${TARGET_RATE_KB_PER_SEC}"}
CAMERA_PORT=${CAMERA_PORT:-8554}
CAMERA_USER=${CAMERA_USER:-user}

SOURCE1="
  videotestsrc name=src num-buffers=300 \
! video/x-raw,width=640,height=480,framerate=30/1 \
! videoconvert \
! x264enc key-int-max=60 tune=zerolatency bitrate=${BITRATE_KILOBITS_PER_SEC} speed-preset=ultrafast \
! h264parse \
! video/x-h264,alignment=au \
"

SOURCE2="
rtspsrc \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD}@${CAMERA_ADDRESS}:${CAMERA_PORT}${CAMERA_PATH}" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  rtcp-sync-send-time=false \
! rtph264depay \
! identity eos-after=300 \
! h264parse \
! video/x-h264,alignment=au \
"

SOURCE=${SOURCE2}

export GST_DEBUG_DUMP_DOT_DIR="/tmp/gst-dot/$(basename "${0}" .sh)-h264"
echo rm -rf "${GST_DEBUG_DUMP_DOT_DIR}"
mkdir -p "${GST_DEBUG_DUMP_DOT_DIR}"

gst-launch-1.0 \
${SOURCE} \
! filesink \
  location=${HOME}/test.h264 \


export GST_DEBUG_DUMP_DOT_DIR="/tmp/gst-dot/$(basename "${0}" .sh)-mp4"
echo rm -rf "${GST_DEBUG_DUMP_DOT_DIR}"
mkdir -p "${GST_DEBUG_DUMP_DOT_DIR}"

gst-launch-1.0 \
${SOURCE} \
! mp4mux streamable=true fragment-duration=${FRAGMENT_DURATION_MS} \
! filesink \
  location=${HOME}/test.mp4 \


export GST_DEBUG_DUMP_DOT_DIR="/tmp/gst-dot/$(basename "${0}" .sh)-ts"
echo rm -rf "${GST_DEBUG_DUMP_DOT_DIR}"
mkdir -p "${GST_DEBUG_DUMP_DOT_DIR}"

gst-launch-1.0 \
${SOURCE} \
! mpegtsmux \
! filesink \
  location=${HOME}/test.ts \


wait

${ROOT_DIR}/scripts/dot-to-png.sh /tmp/gst-dot/$(basename "${0}" .sh)-*/*.dot

ls -l ${HOME}/test.h264 ${HOME}/test.mp4 ${HOME}/test.ts
FILESIZE_H264=$(stat -c%s "${HOME}/test.h264")
FILESIZE_MP4=$(stat -c%s "${HOME}/test.mp4")
FILESIZE_TS=$(stat -c%s "${HOME}/test.ts")
MP4_INCREASE_PERCENT=$(echo 100 \* ${FILESIZE_MP4} / ${FILESIZE_H264} - 100.0 | bc -l)
TS_INCREASE_PERCENT=$(echo 100 \* ${FILESIZE_TS} / ${FILESIZE_H264} - 100.0 | bc -l)
