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
  audiotestsrc name=src is-live=true do-timestamp=true num-buffers=100 \
! "audio/x-raw,format=S16LE,layout=interleaved,rate=44100,channels=1" \
! filesink location=/tmp/audio1 \
|& tee /tmp/audio-to-file.log

gst-launch-1.0 \
-v \
  filesrc location=/tmp/audio1 \
! "audio/x-raw,format=S16LE,layout=interleaved,rate=44100,channels=1" \
! audioconvert \
! autoaudiosink \
|& tee /tmp/file-to-audio.log
