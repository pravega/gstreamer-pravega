#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}

gst-launch-1.0 \
-v \
--eos-on-shutdown \
nvarguscamerasrc \
! 'video/x-raw(memory:NVMM),width=640, height=480, framerate=30/1, format=NV12' \
! nvvidconv flip-method=2 \
! nvvidconv \
! nvegltransform \
! nveglglessink -e
