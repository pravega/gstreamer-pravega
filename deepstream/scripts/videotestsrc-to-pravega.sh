#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:5
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-camera8}

export GST_DEBUG=pravegasink:5
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-test1}
SIZE_SEC=5
FPS=30

gst-launch-1.0 \
-v \
videotestsrc name=src is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! x264enc tune=zerolatency \
! mpegtsmux \
! pravegasink stream=examples/${PRAVEGA_STREAM} controller=192.168.1.123:9090
