#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
export GST_DEBUG=INFO

export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/file-to-screen-and-speaker-split
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
filesrc location=/mnt/data/tmp/audio1.ts \
! tsdemux \
! avdec_aac \
! audioconvert \
! audioresample \
! autoaudiosink \
filesrc location=/mnt/data/tmp/av1.ts \
! tsdemux \
! h264parse \
! avdec_h264 \
! videoconvert \
! autovideosink
