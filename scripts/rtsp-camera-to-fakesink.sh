#!/usr/bin/env bash
set -ex

CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
export GST_DEBUG=rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,identity:LOG
#export GST_DEBUG=DEBUG

gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  rtcp-sync-send-time=false \
! rtph264depay \
! h264parse \
! mpegtsmux \
! identity silent=false \
! fakesink \
$* |& tee /tmp/rtsp-camera-to-fakesink.log
