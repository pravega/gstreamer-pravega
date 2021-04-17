#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:LOG
export RUST_BACKTRACE=1
STREAM=${STREAM:-camera8}
FPS=30

gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! x264enc tune=zerolatency key-int-max=${FPS} bitrate=1000 \
! mpegtsmux \
! pravegasink stream=examples/${STREAM} sync=false
