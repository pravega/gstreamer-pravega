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
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
popd
ls -lh ${ROOT_DIR}/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/release:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:5
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-camera9}
BITRATE_KILOBITS_PER_SEC=8000

gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! tee name=t \
t. ! queue \
   ! x264enc tune=zerolatency bitrate=${BITRATE_KILOBITS_PER_SEC} \
   ! mpegtsmux \
   ! tee name=ts \
   ts. ! queue \
       ! pravegasink stream=examples/${PRAVEGA_STREAM} sync=false timestamp-mode=realtime-clock \
   ts. ! queue \
       ! tsdemux \
       ! h264parse \
       ! avdec_h264 \
       ! videoconvert \
       ! textoverlay "text=camera encode+decode" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
       ! autovideosink sync=false \
t. ! queue2 \
   ! textoverlay "text=camera to ${PRAVEGA_STREAM}" valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
   ! autovideosink sync=false
