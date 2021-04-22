#!/usr/bin/env bash

# Record video from an RTSP camera and write to Pravega.

set -ex

ROOT_DIR=$(readlink -f $(dirname $0)/..)
LOG_FILE=/tmp/rtsp-camera-to-pravega.log

pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
popd

export ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
export BUFFER_SIZE_MB=${BUFFER_SIZE_MB:-50}
export CAMERA_ADDRESS=${CAMERA_ADDRESS:-127.0.0.1}
export CAMERA_PASSWORD=${CAMERA_PASSWORD:?Required environment variable not set}
export CAMERA_PATH="/cam/realmonitor?channel=1&subtype=0"
export CAMERA_PORT=${CAMERA_PORT:-8554}
export CAMERA_USER=${CAMERA_USER:-user}
# log level can be INFO, DEBUG, or LOG (verbose)
#export GST_DEBUG=pravegasink:DEBUG,basesink:INFO,rtspsrc:INFO,rtpbin:INFO,rtpsession:INFO,rtpjitterbuffer:INFO
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-tcp://127.0.0.1:9090}
export PRAVEGA_SCOPE=${PRAVEGA_SCOPE:-examples}
export PRAVEGA_STREAM=${PRAVEGA_STREAM:-rtsp1}
export RUST_BACKTRACE=1

${ROOT_DIR}/python_apps/rtsp-camera-to-pravega.py \
$* |& tee ${LOG_FILE}
