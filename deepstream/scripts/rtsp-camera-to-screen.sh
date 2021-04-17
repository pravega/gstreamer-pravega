#!/usr/bin/env bash
set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/../..)
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
export GST_DEBUG="rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO,rtph264depay:INFO,h264parse:INFO,identity:LOG"
export GST_DEBUG_DUMP_DOT_DIR=${ROOT_DIR}/tmp/gst-dot/rtsp-camera-to-screen
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0" \
! rtph264depay \
! decodebin \
! videoconvert \
! autovideosink
