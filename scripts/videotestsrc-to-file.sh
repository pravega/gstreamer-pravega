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
  videotestsrc name=src is-live=true do-timestamp=true num-buffers=30 \
! video/x-raw,width=160,height=120,framerate=30/1 \
! videoconvert \
! clockoverlay "font-desc=Sans, 48" "time-format=%F %T" \
! x264enc tune=zerolatency \
! mpegtsmux \
! filesink location=test30.ts
