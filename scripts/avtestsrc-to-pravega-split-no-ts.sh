#!/usr/bin/env bash

# Generate audio and video and write to Pravega.
# Audio and video will be written to separate Pravega streams without MPEG Transport Streams.
# Playback using pravega-to-screen-and-speaker-split-no-ts.sh.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:LOG
export RUST_BACKTRACE=1
STREAM=${STREAM:-split10}
SIZE_SEC=600
FPS=30

gst-launch-1.0 \
-v \
videotestsrc is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,width=640,height=480,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! x264enc \
! "video/x-h264,stream-format=byte-stream,profile=main" \
! queue \
! pravegasink stream=examples/${STREAM}-v sync=false \
audiotestsrc is-live=true do-timestamp=true \
             samplesperbuffer=$((44100/$FPS)) num-buffers=$(($SIZE_SEC*$FPS)) \
             wave=ticks volume=0.5 marker-tick-period=5 \
! audioconvert \
! "audio/x-raw,rate=44100,channels=2" \
! avenc_aac \
! queue \
! pravegasink stream=examples/${STREAM}-a1 sync=false
