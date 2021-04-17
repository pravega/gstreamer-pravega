#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

gst-launch-1.0 \
-v \
  videotestsrc name=src is-live=true do-timestamp=true num-buffers=30 \
! video/x-raw,width=160,height=120,framerate=30/1 \
! videoconvert \
! clockoverlay "font-desc=Sans, 48" "time-format=%F %T" \
! x264enc tune=zerolatency \
! mpegtsmux \
! filesink location=test30.ts
