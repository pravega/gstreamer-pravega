#!/usr/bin/env bash

# TODO: For an unknown reason, the timestamp appears to progress faster than real time.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:DEBUG,basesink:INFO
export PRAVEGA_VIDEO_LOG=trace
export RUST_LOG=debug
export RUST_BACKTRACE=full
PRAVEGA_CONTROLLER=${PRAVEGA_CONTROLLER:-127.0.0.1:9090}
SCOPE=${SCOPE:-examples}
STREAM=${STREAM:-test1}
ALLOW_CREATE_SCOPE=${ALLOW_CREATE_SCOPE:-true}
SIZE_SEC=10
FPS=30

gst-launch-1.0 \
-v \
videotestsrc name=src is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! x264enc tune=zerolatency key-int-max=${FPS} bitrate=200 \
! mpegtsmux alignment=-1 \
! pravegasink stream=${SCOPE}/${STREAM} controller=${PRAVEGA_CONTROLLER} sync=true allow-create-scope=${ALLOW_CREATE_SCOPE}
