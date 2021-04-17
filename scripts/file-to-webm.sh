#!/usr/bin/env bash
set -ex
gst-launch-1.0 \
-v \
filesrc location=traffic-intersection-4188267.mpg \
! decodebin name=demux \
! videoscale \
! video/x-raw,width=320,height=200 \
! vp8enc ! queue ! webmmux name=mux \
! filesink location=go8.webm \
|& tee go8.log
