#!/usr/bin/env bash

# Record audio and video from an RTSP camera and write to Pravega.
# Audio and video are stored together in the same MPEG Transport Stream.

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:LOG,basesink:INFO,rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,identity:LOG
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtspav1}
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}

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
! identity silent=false \
! pravegasink \
  stream=examples/${PRAVEGA_STREAM} \
  timestamp-mode=ntp \
  sync=false \
$* |& tee /tmp/rtsp-camera-to-pravega-with-audio.log
