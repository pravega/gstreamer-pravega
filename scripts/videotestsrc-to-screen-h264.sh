#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

gst-launch-1.0 \
-v \
  videotestsrc name=src is-live=true do-timestamp=true \
! video/x-raw,width=160,height=120,framerate=30/1 \
! x264enc \
! mpegtsmux \
! decodebin \
! videoconvert \
! autovideosink
