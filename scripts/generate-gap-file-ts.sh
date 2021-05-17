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
export GST_DEBUG=identity:LOG
SIZE_SEC=5
FPS=30

gst-launch-1.0 \
-v \
videotestsrc pattern=blue num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,width=320,height=240,framerate=30/1" \
! videoconvert \
! x264enc \
! mpegtsmux name=mux \
! filesink location=${ROOT_DIR}/pravega-video-server/static/gap-${SIZE_SEC}s.ts
