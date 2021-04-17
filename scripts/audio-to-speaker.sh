#!/usr/bin/env bash
# Generate audio and play on speakers.
gst-launch-1.0 audiotestsrc ! autoaudiosink
