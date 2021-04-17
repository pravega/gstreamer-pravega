#!/usr/bin/env bash
# View .dot files produced by gst-launch-1.0.
set -e
png_file=/tmp/$(basename "$1").png
dot "$1" -Tpng -o "${png_file}"
eog "${png_file}"
