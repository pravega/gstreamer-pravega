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

SIZE_SEC=3600
FPS=30
TARGET_RATE_KB_PER_SEC=2000
TMPFILE=/tmp/benchmark-data-${SIZE_SEC}-${TARGET_RATE_KB_PER_SEC}.ts
