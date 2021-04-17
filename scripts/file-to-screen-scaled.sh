#!/usr/bin/env bash
gst-launch-1.0 \
-v \
filesrc location=traffic-intersection-4188267.mpg \
! decodebin \
! identity \
! videoscale \
! video/x-raw,width=320,height=200,format=I420 \
! videoconvert \
! autovideosink \
|& tee go7.log
