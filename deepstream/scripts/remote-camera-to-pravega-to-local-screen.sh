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
ssh ${SSH_OPTS} ${SSH_HOST} "pkill gst-launch ; true"
deepstream/scripts/remote-build.sh
export PRAVEGA_STREAM=${PRAVEGA_STREAM:-$(uuidgen)}
ssh ${SSH_OPTS} ${SSH_HOST} "cd ~/gstreamer-pravega && \
    PRAVEGA_STREAM=${PRAVEGA_STREAM} deepstream/scripts/camera-to-pravega.sh \
    >& /tmp/gstreamer-pravega.log" &
scripts/pravega-to-screen.sh ; true
ssh ${SSH_OPTS} ${SSH_HOST} "pkill gst-launch ; true"
