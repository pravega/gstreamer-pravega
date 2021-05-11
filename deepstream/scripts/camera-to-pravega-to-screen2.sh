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
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
ls -lh ${ROOT_DIR}/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasink:5
export RUST_BACKTRACE=1
PRAVEGA_STREAM=${PRAVEGA_STREAM:-$(uuidgen)}
PRAVEGA_CONTROLLER_URI=192.168.1.123:9090

pkill gst-launch || true

gst-launch-1.0 \
-v \
--eos-on-shutdown \
nvarguscamerasrc \
! 'video/x-raw(memory:NVMM),width=640, height=480, framerate=30/1, format=NV12' \
! nvvidconv flip-method=2 \
! nvv4l2h264enc maxperf-enable=1 preset-level=1 control-rate=0 bitrate=400000 \
! mpegtsmux \
! timestampadd \
! pravegasink stream=examples/${PRAVEGA_STREAM} controller=${PRAVEGA_CONTROLLER_URI} \
&

gst-launch-1.0 \
-v \
--eos-on-shutdown \
pravegasrc stream=examples/${PRAVEGA_STREAM} controller=${PRAVEGA_CONTROLLER_URI} \
! timestampremove \
! tsdemux \
! h264parse \
! omxh264dec \
! nvegltransform \
! nveglglessink -e sync=false
