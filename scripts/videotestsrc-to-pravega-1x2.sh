#!/usr/bin/env bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

# This writes two independent H264 video MPEG Transport Streams, as if from 2 different cameras.
# These can be played back concurrently with pravega-to-screen-1x2.sh.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
popd
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
pushd ${ROOT_DIR}/apps
cargo build --bin launch
popd
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:INFO,basesink:INFO
export RUST_LOG=info
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-group1}
SIZE_SEC=60
FPS=30
WIDTH=320
HEIGHT=240

${ROOT_DIR}/apps/target/debug/launch \
videotestsrc is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) pattern=smpte \
! "video/x-raw,format=YUY2,width=$WIDTH,height=$HEIGHT,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! queue \
! x264enc tune=zerolatency key-int-max=${FPS} bitrate=200 \
! queue \
! mpegtsmux \
! pravegasink stream=examples/${PRAVEGA_STREAM}-v1 sync=false \
>& /mnt/data/logs/videotestsrc-to-pravega-1x2-v1.log &

${ROOT_DIR}/apps/target/debug/launch \
videotestsrc is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) pattern=ball \
! "video/x-raw,format=YUY2,width=$WIDTH,height=$HEIGHT,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! queue \
! x264enc tune=zerolatency key-int-max=${FPS} bitrate=200 \
! queue \
! mpegtsmux \
! pravegasink stream=examples/${PRAVEGA_STREAM}-v2 sync=false \
>& /mnt/data/logs/videotestsrc-to-pravega-1x2-v2.log &

wait
