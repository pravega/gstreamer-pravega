#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
export GST_DEBUG=identity:LOG
SIZE_SEC=11
FPS=30

gst-launch-1.0 \
-v \
videotestsrc is-live=true do-timestamp=true num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,width=1920,height=1280,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! x264enc \
! queue ! mux. \
audiotestsrc is-live=true do-timestamp=true \
             samplesperbuffer=$((44100/$FPS)) num-buffers=$(($SIZE_SEC*$FPS)) \
             wave=ticks volume=0.5 marker-tick-period=5 \
! audioconvert \
! "audio/x-raw,rate=44100,channels=2" \
! avenc_aac \
! queue ! mux. \
mpegtsmux name=mux \
! identity silent=false \
! filesink location=/mnt/data/tmp/av1.ts
