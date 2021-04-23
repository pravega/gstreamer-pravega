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
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=30/1" \
! videoconvert \
! coloreffects preset=xray \
! videoconvert \
! autovideosink sync=false
