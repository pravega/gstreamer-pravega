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

for comp in apps gst-plugin-pravega pravega-video pravega-video-server; do
    pushd ${ROOT_DIR}/$comp

    set +e
    path_to_cargo2junit=$(which cargo2junit)
    set -e
    if [ -x "$path_to_cargo2junit" ] ; then
        cargo test --locked --release $* -- -Z unstable-options --format json | cargo2junit | tee junit.xml
    else
        cargo test --locked --release
    fi

    popd
done
