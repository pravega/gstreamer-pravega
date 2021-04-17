#!/usr/bin/env bash
set -ex

CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
export GST_DEBUG="rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,rtph264depay:LOG,h264parse:LOG,\
rtpaacdepay:LOG,aacparse:LOG,
mpegtsmux:LOG,mpegtsbase:LOG,mpegtspacketizer:LOG,filesink:LOG,basesink:INFO,identity:LOG"
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/rtsp-camera-to-file-audio-only
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}


gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  name=src \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  rtcp-sync-send-time=false \
! rtpmp4gdepay \
! aacparse \
! mpegtsmux \
! identity silent=false \
! filesink location=/mnt/data/tmp/rtsp-camera.ts \
|& tee /mnt/data/logs/rtsp-camera-to-file-audio-only.log
