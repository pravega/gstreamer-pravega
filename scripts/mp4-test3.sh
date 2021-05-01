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
LOG_FILE="/tmp/$(basename "${0}" .sh).log"
export GST_DEBUG=qtmux:INFO,FIXME
# 2001-02-03T04:00:00.000000000Z (981172837000000000 ns, 272548:00:37.000000000)
FIRST_TS=981172837000000000
FRAGMENT_DURATION_MS=15
#PLAY_OFFSET=$(( -${FIRST_TS} - 1000000000000 ))
PLAY_OFFSET=0

gst-launch-1.0 \
-v \
  videotestsrc name=src timestamp-offset=${FIRST_TS} num-buffers=360 \
! video/x-raw,width=160,height=120,framerate=30/1 \
! videoconvert \
! clockoverlay "font-desc=Sans, 48" "time-format=%F %T" \
! timeoverlay valignment=bottom "font-desc=Sans 48px" \
! videoconvert \
! x264enc key-int-max=60 tune=zerolatency \
! mp4mux streamable=true fragment-duration=${FRAGMENT_DURATION_MS} \
! identity silent=false name=mp4 \
! decodebin \
! identity silent=false name=decoded \
! videoconvert \
! autovideosink sync=true ts-offset=${PLAY_OFFSET} \
|& tee ${LOG_FILE}
