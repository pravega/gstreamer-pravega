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
pushd ${ROOT_DIR}/gst-plugin-rs/tutorial
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-rs/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-rs/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=4
export RUST_BACKTRACE=full

gst-launch-1.0 --version

gst-launch-1.0 \
-v \
rssinesrc  !  audioconvert  !  monoscope  !  timeoverlay  !  navseek  !  autovideosink
