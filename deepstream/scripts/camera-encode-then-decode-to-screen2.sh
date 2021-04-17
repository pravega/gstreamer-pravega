#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}
BITRATE_KILOBITS_PER_SEC=8000

gst-launch-1.0 \
-v \
--eos-on-shutdown \
nvarguscamerasrc \
! 'video/x-raw(memory:NVMM),width=640, height=480, framerate=30/1, format=NV12' \
! nvvidconv flip-method=2 \
! 'video/x-raw,width=640, height=480, framerate=30/1' \
! x264enc tune=zerolatency bitrate=${BITRATE_KILOBITS_PER_SEC} \
! mpegtsmux \
! tsdemux \
! h264parse \
! omxh264dec \
! nvegltransform \
! nveglglessink -e sync=false
