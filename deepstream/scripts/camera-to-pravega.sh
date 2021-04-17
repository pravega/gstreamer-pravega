#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:DEBUG
export RUST_BACKTRACE=1
STREAM=${STREAM:-camera9}
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-192.168.1.123:9090}
FPS=21
BITRATE_KILOBYTES_PER_SEC=1000
BITRATE_BITS_PER_SEC=$(( 8000 * ${BITRATE_KILOBYTES_PER_SEC} ))

gst-launch-1.0 \
-v \
--eos-on-shutdown \
nvarguscamerasrc \
! "video/x-raw(memory:NVMM),width=3264, height=2464, framerate=${FPS}/1, format=NV12" \
! nvvidconv flip-method=2 \
! nvv4l2h264enc maxperf-enable=1 preset-level=1 control-rate=1 bitrate=${BITRATE_BITS_PER_SEC} \
! mpegtsmux \
! pravegasink stream=examples/${STREAM} controller=${PRAVEGA_CONTROLLER} \
|& tee /tmp/camera-to-pravega.log
