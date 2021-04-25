#!/usr/bin/env python3

#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

#
# Usage:
#   GST_PLUGIN_PATH=$PWD/..:$GST_PLUGIN_PATH \
#   gst-launch-1.0 videotestsrc ! identity_py ! autovideosink
#
# See also https://mathieuduponchelle.github.io/2018-02-15-Python-Elements-2.html
#

import gi
gi.require_version('GLib', '2.0')
gi.require_version('GObject', '2.0')
gi.require_version('Gst', '1.0')
gi.require_version('GstBase', '1.0')
from gi.repository import GLib, GObject, Gst, GstBase


class Identity(GstBase.BaseTransform):
    __gstmetadata__ = (
        'Identity Python',
        'Transform', \
        'Simple identity element written in Python',
        'Claudio Fahey')

    __gsttemplates__ = (Gst.PadTemplate.new("src",
                                           Gst.PadDirection.SRC,
                                           Gst.PadPresence.ALWAYS,
                                           Gst.Caps.new_any()),
                       Gst.PadTemplate.new("sink",
                                           Gst.PadDirection.SINK,
                                           Gst.PadPresence.ALWAYS,
                                           Gst.Caps.new_any()))

    def do_transform_ip(self, buffer):
        Gst.info("timestamp: %s" % (Gst.TIME_ARGS(buffer.pts)))
        return Gst.FlowReturn.OK


__gstelementfactory__ = ("identity_py", Gst.Rank.NONE, Identity)
