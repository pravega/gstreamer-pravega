#!/usr/bin/env bash

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO
export RUST_BACKTRACE=1
export TZ=UTC
STREAM=${STREAM:-hls1}
SIZE_SEC=604800
FPS=30
KEY_FRAME_INTERVAL=$((5*$FPS))

gst-launch-1.0 \
-v \
videotestsrc name=src is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=640,height=480,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! queue \
! x264enc key-int-max=${KEY_FRAME_INTERVAL} tune=zerolatency speed-preset=medium bitrate=500 \
! queue \
! mpegtsmux alignment=-1 \
! pravegasink stream=examples/${STREAM} controller=127.0.0.1:9090 seal=false sync=false
