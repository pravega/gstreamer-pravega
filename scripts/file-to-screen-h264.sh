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
export GST_DEBUG="filesrc:6,basesrc:6,mpegtsbase:6,mpegtspacketizer:6"
export RUST_BACKTRACE=1
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/file-to-screen-h264
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
filesrc location=/mnt/data/tmp/test3.ts \
! queue \
! tsdemux \
! h264parse \
! avdec_h264 \
! videoconvert \
! autovideosink
