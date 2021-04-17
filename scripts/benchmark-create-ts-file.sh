#!/usr/bin/env bash
#
# Create a MPEG transport stream for benchmarking.
#

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)

source ${ROOT_DIR}/scripts/benchmark-config.sh

BITRATE_KILOBITS_PER_SEC=$(( ${TARGET_RATE_KB_PER_SEC} * 8 ))

time gst-launch-1.0 \
-v \
videotestsrc name=src num-buffers=$(($SIZE_SEC*$FPS)) \
! "video/x-raw,format=YUY2,width=3840,height=2160,framerate=${FPS}/1" \
! videoconvert \
! x264enc key-int-max=${FPS} speed-preset=ultrafast bitrate=${BITRATE_KILOBITS_PER_SEC} \
! mpegtsmux alignment=-1 \
! filesink location=${TMPFILE}

ls -lh ${TMPFILE}
FILESIZE=$(stat -c%s "${TMPFILE}")
THROUGHPUT_KB_PER_SEC=$(( ${FILESIZE} / ${SIZE_SEC} / 1000 ))
echo Actual throughput of generated file: ${THROUGHPUT_KB_PER_SEC} KB/s
