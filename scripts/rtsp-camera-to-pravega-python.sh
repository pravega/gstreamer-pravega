#!/usr/bin/env bash

# Record video from an RTSP camera and write to Pravega.

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
LOG_FILE=/tmp/rtsp-camera-to-pravega.log

pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/release/*.so
popd
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}

# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO
export RUST_BACKTRACE=1
export STREAM=${STREAM:-rtsp1}
CAMERA_USER=${CAMERA_USER:-user}
CAMERA_IP=${CAMERA_IP:-127.0.0.1}
CAMERA_PORT=${CAMERA_PORT:-8554}

${ROOT_DIR}/python_apps/rtsp-camera-to-pravega.py \
--controller 127.0.0.1:9090 \
--scope examples \
--source-uri "rtsp://${CAMERA_USER}:${CAMERA_PASSWORD:?Required environment variable not set}@${CAMERA_IP}:${CAMERA_PORT}/cam/realmonitor?channel=1&subtype=0" \
$* |& tee ${LOG_FILE}
