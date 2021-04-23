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

gst-launch-1.0 \
-v \
filesrc location=traffic-intersection-4188267.mpg \
! decodebin \
! identity \
! videoscale \
! video/x-raw,width=320,height=200,format=I420 \
! videoconvert \
! autovideosink \
|& tee go7.log
