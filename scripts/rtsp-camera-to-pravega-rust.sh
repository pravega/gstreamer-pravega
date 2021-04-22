#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
LOG_FILE=/mnt/data/logs/rtsp-camera-to-pravega-rust.log
# log level can be INFO or LOG (verbose)
export GST_DEBUG="rtspsrc:LOG,rtpbin:LOG,rtpsession:LOG,rtpjitterbuffer:LOG,rtpsource:LOG,rtph264depay:LOG,\
h264parse:LOG,mpegtsmux:LOG,mpegtsbase:LOG,mpegtspacketizer:LOG,identity:LOG,pravegasink:LOG,basesink:INFO"
export RUST_LOG=info
export RUST_BACKTRACE=1
STREAM=${STREAM:-rtsp3}
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-127.0.0.1:9090}
CAMERA_USER=${CAMERA_USER:-admin}
CAMERA_IP=${CAMERA_IP:-192.168.1.102}
RTSP_URL="rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}/cam/realmonitor?channel=1&subtype=0"
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/rtsp-camera-to-pravega-rust
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
pushd ${ROOT_DIR}/apps
cargo run --release --bin rtsp-camera-to-pravega -- --location "${RTSP_URL}" --stream examples/${STREAM} \
  --controller ${PRAVEGA_CONTROLLER_URI} $* \
  |& rotatelogs -L ${LOG_FILE} -p ${ROOT_DIR}/scripts/rotatelogs-compress.sh ${LOG_FILE} 1G
popd
