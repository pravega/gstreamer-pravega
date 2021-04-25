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
export GST_DEBUG=INFO

gst-launch-1.0 \
-v \
filesrc location=/mnt/data/tmp/av1.ts \
! tsdemux name=demuxer \
demuxer. ! queue \
         ! avdec_aac \
         ! audioconvert \
         ! audioresample \
         ! autoaudiosink \
demuxer. ! h264parse \
         ! avdec_h264 \
         ! videoconvert \
         ! autovideosink
