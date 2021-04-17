#!/usr/bin/env bash
set -ex
gst-launch-1.0 \
-v \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=30/1" \
! videoconvert \
! coloreffects preset=xray \
! videoconvert \
! autovideosink sync=false
