#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
export GST_DEBUG="filesrc:6,basesrc:6,mpegtsbase:6,mpegtspacketizer:6"
export RUST_BACKTRACE=1
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/file-to-screen-h264
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
filesrc location=/mnt/data/tmp/test3.ts \
! queue \
! tsdemux \
! h264parse \
! avdec_h264 \
! videoconvert \
! autovideosink
