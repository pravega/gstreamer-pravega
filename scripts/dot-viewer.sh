#!/usr/bin/env bash

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

# View .dot files produced by gst-launch-1.0.
set -e
png_file=/tmp/$(basename "$1").png
dot "$1" -Tpng -o "${png_file}"
eog "${png_file}"
