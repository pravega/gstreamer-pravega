#!/usr/bin/env bash
set -ex
gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=160,height=90,framerate=5/1" \
! videoconvert \
! clockoverlay font-desc=\"Sans, 48\" time-format=\"%F %T\" \
! tee name=t \
t. ! queue \
   ! x264enc key-int-max=5 \
   ! mpegtsmux \
   ! filesink location=test3.ts \
t. ! queue2 \
   ! autovideosink \
