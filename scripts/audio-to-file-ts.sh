#!/usr/bin/env bash

# Generate audio and write to file as an MPEG Transport Stream.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
export GST_DEBUG=INFO
SIZE_SEC=11
FPS=30

gst-launch-1.0 \
-v \
audiotestsrc is-live=true do-timestamp=true \
             samplesperbuffer=$((44100/$FPS)) num-buffers=$(($SIZE_SEC*$FPS)) \
             wave=sine volume=0.5 marker-tick-period=5 \
! "audio/x-raw,rate=44100,channels=2" \
! audioconvert \
! avenc_aac \
! mpegtsmux \
! filesink location=/mnt/data/tmp/audio1.ts \
|& tee /tmp/audio-to-file-ts.log

gst-launch-1.0 \
-v \
  filesrc location=/mnt/data/tmp/audio1.ts \
! tsdemux \
! avdec_aac \
! audioconvert \
! audioresample \
! autoaudiosink \
|& tee /tmp/file-to-audio-ts.log
