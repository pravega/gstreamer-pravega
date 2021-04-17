#!/usr/bin/env bash
set -ex

CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
export GST_DEBUG=rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO,identity:INFO
#export GST_DEBUG=DEBUG
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/rtsp-audio-to-speaker
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
playbin "uri=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0" \
$* |& tee /tmp/rtsp-audio-to-speaker.log
