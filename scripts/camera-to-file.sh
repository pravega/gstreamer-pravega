#!/usr/bin/env bash
set -ex
gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=160,height=90,framerate=5/1" \
! videoconvert \
! x264enc key-int-max=5 \
! mpegtsmux \
! filesink location=test2.ts
