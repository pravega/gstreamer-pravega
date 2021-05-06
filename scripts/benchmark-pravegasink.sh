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

#
# Benchmark throughput of pravegasink.
# Before running this script, run benchmark-create-ts-file.sh to generate the source file.
#
# To simulate the write pattern of ingestion from a live source, 
# we parse the transport stream with tsparse to produce buffers containing the
# desired number of 188-byte frames.
# The number of frames is given by the alignment parameter.
# An alignment of 21 frames will result in approximately 4 KiB events and produces good results.
# tsparse will also identify key frames (random access indicator), which will cause
# pravegasink to flush the data stream and write an index record.
#
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
popd
ls -lh ${ROOT_DIR}/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/target/release:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasink:DEBUG"
export RUST_BACKTRACE=1

source ${ROOT_DIR}/scripts/benchmark-config.sh

PRAVEGA_STREAM=${PRAVEGA_STREAM:-$(uuidgen)}

ls -l ${TMPFILE}

T0=`date +%s%N`

time gst-launch-1.0 \
-v \
filesrc location=${TMPFILE} \
! tsparse \
  alignment=21 \
  split-on-rai=true \
  set-timestamps=true \
! queue \
! pravegasink stream=examples/${PRAVEGA_STREAM} \
  buffer-size=$(( 128 * 1024 )) \
  index-min-sec=600.0 \
  index-max-sec=600.0 \
  sync=false  

T1=`date +%s%N`
DT_MILLIS=$(( ($T1 - $T0) / 1000 / 1000 ))
FILESIZE=$(stat -c%s "${TMPFILE}")
THROUGHPUT_KB_PER_SEC=$(( ${FILESIZE} / ${DT_MILLIS} ))
echo PRAVEGA_STREAM=${PRAVEGA_STREAM}
echo Throughput of pravegasink: ${THROUGHPUT_KB_PER_SEC} KB/s
