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

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
popd
ls -lh ${ROOT_DIR}/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/release:${GST_PLUGIN_PATH}
export RUST_BACKTRACE=1

PRAVEGA_STREAM=${PRAVEGA_STREAM:-$(uuidgen)}
BITRATE_KILOBITS_PER_SEC=200
SIZE_SEC=4
FPS=30
export GST_DEBUG="pravegasrc:4,timestampremove:5,pravegasink:5,mpegtsbase:4,mpegtspacketizer:4"

export GST_DEBUG_FILE=


gst-launch-1.0 \
-v \
  videotestsrc name=src is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=${FPS}/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! videoconvert \
! x264enc tune=zerolatency \
! mpegtsmux alignment=0 \
! timestampadd \
! pravegasink stream=examples/${PRAVEGA_STREAM} timestamp-mode=realtime-clock

export GST_DEBUG="GST_TRACER:7,pravegasrc:4,timestampremove:5,pravegasink:5,mpegtsbase:4,mpegtspacketizer:4"
export GST_DEBUG_FILE=trace.log
export GST_TRACERS="latency(flags=pipeline+element+reported)"

gst-launch-1.0 \
-v \
  pravegasrc stream=examples/${PRAVEGA_STREAM} \
! timestampremove \
! tsdemux \
! h264parse \
! avdec_h264 \
! videoconvert \
! textoverlay "text=from ${PRAVEGA_STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! autovideosink sync=false
