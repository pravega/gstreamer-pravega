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
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo deb
DEB_FILE=${ROOT_DIR}/target/debian/gst-plugin-pravega_0.7.0_arm64.deb
ls -lh ${DEB_FILE}
sudo dpkg -i ${DEB_FILE}
ls -lh /usr/lib/aarch64-linux-gnu/gstreamer-1.0/libgstpravega.so
gst-inspect-1.0 pravega
