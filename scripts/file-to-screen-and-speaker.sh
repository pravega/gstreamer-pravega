#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
export GST_DEBUG=INFO

gst-launch-1.0 \
-v \
filesrc location=/mnt/data/tmp/av1.ts \
! tsdemux name=demuxer \
demuxer. ! queue \
         ! avdec_aac \
         ! audioconvert \
         ! audioresample \
         ! autoaudiosink \
demuxer. ! h264parse \
         ! avdec_h264 \
         ! videoconvert \
         ! autovideosink
