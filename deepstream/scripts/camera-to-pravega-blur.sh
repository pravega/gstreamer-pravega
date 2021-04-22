#!/usr/bin/env bash
# Capture from camera, blur faces, and write to Pravega.
# The OpenCV faceblur element uses the CPU so the video frames must be transferred between the GPU and CPU.
# Prerequisite: sudo apt-get install gstreamer1.0-opencv
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:DEBUG,INFO
export RUST_BACKTRACE=1
STREAM=${STREAM:-camera9}
PRAVEGA_CONTROLLER_URI=${PRAVEGA_CONTROLLER_URI:-192.168.1.123:9090}
FPS=15
BITRATE_KILOBYTES_PER_SEC=500
BITRATE_BITS_PER_SEC=$(( 8000 * ${BITRATE_KILOBYTES_PER_SEC} ))

gst-launch-1.0 \
-v \
--eos-on-shutdown \
nvarguscamerasrc \
! "video/x-raw(memory:NVMM),width=1280,height=720,framerate=${FPS}/1,format=NV12" \
! nvvidconv flip-method=0 \
! "video/x-raw" \
! videoconvert \
! "video/x-raw,format=RGB" \
! faceblur profile=/usr/share/opencv4/haarcascades/haarcascade_frontalface_default.xml \
! videoconvert \
! "video/x-raw,format=NV12" \
! nvvidconv \
! "video/x-raw(memory:NVMM)" \
! nvv4l2h264enc maxperf-enable=1 preset-level=1 control-rate=1 bitrate=${BITRATE_BITS_PER_SEC} \
! mpegtsmux \
! pravegasink stream=examples/${STREAM} controller=${PRAVEGA_CONTROLLER_URI} \
|& tee /tmp/camera-to-pravega-blur.log
