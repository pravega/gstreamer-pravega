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
scripts/rsync.sh
pushd ${ROOT_DIR}/gst-plugin-pravega
ssh ${SSH_OPTS} ${SSH_HOST} "cd ~/gstreamer-pravega/gst-plugin-pravega && cargo build && ls -lh target/debug/*.so"
