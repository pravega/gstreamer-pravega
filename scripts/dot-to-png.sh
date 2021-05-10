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

# Convert .dot files produced by gst-launch-1.0 to PNG files.

for dot_file in "$@"; do
    png_file="$(dirname "${dot_file}")/$(basename "${dot_file}" .dot).png"
    echo "${dot_file} => ${png_file}"
    dot "${dot_file}" -Tpng -o "${png_file}" &
done
wait
