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
gst-launch-1.0 \
-v \
--eos-on-shutdown \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=160,height=90,framerate=5/1" \
! videoconvert \
! x264enc key-int-max=5 \
! mpegtsmux \
! filesink location=test2.ts
