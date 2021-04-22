#!/usr/bin/env bash
set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
CONTAINER_NAME=$(basename -s .sh $0)
LOG_FILE=/mnt/data/logs/${CONTAINER_NAME}.log
export GST_DEBUG=pravegasink:LOG,basesink:INFO,rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,identity:LOG
export RUST_BACKTRACE=1
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-192.168.1.123:9090}
STREAM=${STREAM:-rtsp1}
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
CAMERA_PORT=${CAMERA_PORT:-554}

docker run --rm \
--name ${CONTAINER_NAME} \
--log-driver json-file --log-opt max-size=10m --log-opt max-file=2 \
--network host \
-e GST_DEBUG \
-e RUST_BACKTRACE \
pravega/gstreamer:pravega-dev \
gst-launch-1.0 \
-v \
--eos-on-shutdown \
rtspsrc \
  "location=rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}:${CAMERA_PORT}/cam/realmonitor?channel=1&subtype=0" \
  buffer-mode=none \
  drop-messages-interval=0 \
  drop-on-latency=true \
  latency=2000 \
  ntp-sync=true \
  ntp-time-source=running-time \
  rtcp-sync-send-time=false \
! identity name=identity-from-rtspsrc silent=false \
! rtph264depay \
! h264parse \
! "video/x-h264,alignment=au" \
! mpegtsmux \
! identity name=from-mpegtsmux silent=false \
! queue max-size-buffers=0 max-size-bytes=10485760 max-size-time=0 silent=true leaky=downstream \
! identity name=from-queue silent=false \
! pravegasink \
  stream=examples/${STREAM} \
  controller=${PRAVEGA_CONTROLLER_URI} \
  timestamp-mode=ntp \
  sync=false \
$* |& rotatelogs -L ${LOG_FILE} -p ${ROOT_DIR}/scripts/rotatelogs-compress.sh ${LOG_FILE} 1G
