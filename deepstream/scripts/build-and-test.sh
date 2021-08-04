#!/bin/bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

# This can be run in the DeepStream Development Pod to build gstreamer-pravega and run a sample DeepStream application.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
cargo build --package gst-plugin-pravega --locked --release
cargo build --package pravega_protocol_adapter --locked --release
sudo ln -f -s -v ${ROOT_DIR}/target/release/libgstpravega.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/
sudo ln -f -s -v ${ROOT_DIR}/target/release/libnvds_pravega_proto.so /opt/nvidia/deepstream/deepstream/lib/
gst-inspect-1.0 pravega

export ADD_MESSAGE_WHEN_NO_OBJECTS_FOUND=true
export ALLOW_CREATE_SCOPE=false
#export GST_DEBUG=INFO,pravegasrc:LOG,pravegasink:LOG,pravegatc:TRACE,fragmp4pay:LOG,qtdemux:LOG,h264parse:LOG,v4l2:LOG
export HEALTH_CHECK_ENABLED=true
export HEALTH_CHECK_IDLE_SECONDS=15
export INPUT_STREAM=camera002
#export INPUT_STREAM=camera-claudio-07
#export LOG_LEVEL=10
export OUTPUT_METADATA_STREAM=metadata-claudio-08
#export OUTPUT_VIDEO_STREAM=osd-claudio-14
export PYTHONPATH=${ROOT_DIR}/python_apps/lib
export RECOVERY_TABLE=metadata-recovery-table-4
#export RECOVERY_TABLE=osd-recovery-table-2
#export RUST_LOG=nvds_pravega_proto=trace,info
export START_MODE=latest
deepstream/python_apps/deepstream-pravega-demos/pravega-to-object-detection-to-pravega.py >& /tmp/app.log

# export INPUT_STREAM=camera002
# export OUTPUT_STREAM=${INPUT_STREAM}-copy3
# export RECOVERY_TABLE=copy-recovery-table-8
# python_apps/pravega-to-pravega.py # >& /tmp/app.log
