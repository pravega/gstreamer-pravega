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

# Generate audio and video and write to Pravega.
# Audio and video will be written to separate Pravega streams as MPEG Transport Streams.
# Playback using pravega-to-screen-and-speaker-split.sh.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:LOG
export RUST_BACKTRACE=1
export TZ=UTC
PRAVEGA_STREAM=${PRAVEGA_STREAM:-split1}
SIZE_SEC=3600
FPS=30
KEY_FRAME_INTERVAL=$((5*$FPS))

gst-launch-1.0 \
-v \
videotestsrc is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,width=640,height=480,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! x264enc key-int-max=${KEY_FRAME_INTERVAL} tune=zerolatency speed-preset=medium bitrate=500 \
! mpegtsmux \
! pravegasink stream=examples/${PRAVEGA_STREAM}-v sync=false timestamp-mode=realtime-clock \
audiotestsrc is-live=true do-timestamp=true \
             samplesperbuffer=$((44100/$FPS)) num-buffers=$(($SIZE_SEC*$FPS)) \
             wave=ticks volume=0.5 marker-tick-period=5 \
! audioconvert \
! "audio/x-raw,rate=44100,channels=2" \
! avenc_aac \
! mpegtsmux \
! pravegasink stream=examples/${PRAVEGA_STREAM}-a1 sync=false timestamp-mode=realtime-clock \
audiotestsrc is-live=true do-timestamp=true \
             samplesperbuffer=$((44100/$FPS)) num-buffers=$(($SIZE_SEC*$FPS)) \
             wave=sine \
! audioconvert \
! "audio/x-raw,rate=44100,channels=2" \
! avenc_aac \
! mpegtsmux \
! pravegasink stream=examples/${PRAVEGA_STREAM}-a2 sync=false timestamp-mode=realtime-clock
