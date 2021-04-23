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

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! tee name=t \
t. ! queue2 \
   ! x264enc tune=zerolatency \
   ! mpegtsmux \
   ! tsdemux \
   ! h264parse \
   ! avdec_h264 \
   ! videoconvert \
   ! textoverlay text=DECODED valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
   ! autovideosink sync=false \
t. ! queue2 \
   ! textoverlay text=LIVE valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
   ! autovideosink sync=false \
