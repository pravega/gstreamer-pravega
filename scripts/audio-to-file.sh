#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

export GST_DEBUG=INFO

gst-launch-1.0 \
-v \
  audiotestsrc name=src is-live=true do-timestamp=true num-buffers=100 \
! "audio/x-raw,format=S16LE,layout=interleaved,rate=44100,channels=1" \
! filesink location=/tmp/audio1 \
|& tee /tmp/audio-to-file.log

gst-launch-1.0 \
-v \
  filesrc location=/tmp/audio1 \
! "audio/x-raw,format=S16LE,layout=interleaved,rate=44100,channels=1" \
! audioconvert \
! autoaudiosink \
|& tee /tmp/file-to-audio.log
