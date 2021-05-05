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

pushd ${ROOT_DIR}/apps
cargo test $*
popd

pushd ${ROOT_DIR}/gst-plugin-pravega
cargo test $*
popd

pushd ${ROOT_DIR}/pravega-video
cargo test $*
popd

pushd ${ROOT_DIR}/pravega-video-server
cargo test $*
popd
