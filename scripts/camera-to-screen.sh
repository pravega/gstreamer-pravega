#!/usr/bin/env bash
# Note: You can use gst-device-monitor-1.0 to view installed cameras and capabilities.
set -ex
gst-launch-1.0 \
-v \
v4l2src do-timestamp=TRUE \
! "video/x-raw,format=YUY2,width=320,height=180,framerate=30/1" \
! videoconvert \
! clockoverlay "font-desc=Sans 48px" "time-format=%F %T" shaded-background=true \
! timeoverlay valignment=bottom "font-desc=Sans 48px" shaded-background=true \
! textoverlay text=LIVE valignment=baseline halignment=right "font-desc=Sans 24px" shaded-background=true \
! autovideosink sync=false
