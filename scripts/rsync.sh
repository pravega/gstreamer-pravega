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

# Copy this repo and related files to a remote system.
set -ex

: ${SSH_HOST?"You must export SSH_HOST"}

rsync -e "ssh ${SSH_OPTS}" -v -r -c --delete --exclude-from .gitignore . ${SSH_HOST}:~/gstreamer-pravega
