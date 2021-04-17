#!/usr/bin/env python3
#
# This can be used with videotestsrc to simulate a reproducible live feed. The PTS of the first buffer will be fixed.
#
# Usage:
#   GST_PLUGIN_PATH=$PWD/..:$GST_PLUGIN_PATH \
#   gst-launch-1.0 videotestsrc is-live=true ! live_feed_simulator ! autovideosink
#
# See also https://mathieuduponchelle.github.io/2018-02-15-Python-Elements-2.html
#

import gi
gi.require_version("GLib", "2.0")
gi.require_version("GObject", "2.0")
gi.require_version("Gst", "1.0")
gi.require_version("GstBase", "1.0")
from gi.repository import GLib, GObject, Gst, GstBase


class LiveFeedSimulator(GstBase.BaseTransform):
    __gstmetadata__ = (
        "Live Feed Simulator",
        "Transform",
        "This can be used with videotestsrc to simulate a reproducible live feed. The PTS of the first buffer will be fixed.",
        "Claudio Fahey")

    __gsttemplates__ = (Gst.PadTemplate.new("src",
                                           Gst.PadDirection.SRC,
                                           Gst.PadPresence.ALWAYS,
                                           Gst.Caps.new_any()),
                       Gst.PadTemplate.new("sink",
                                           Gst.PadDirection.SINK,
                                           Gst.PadPresence.ALWAYS,
                                           Gst.Caps.new_any()))

    def __init__(self):
        GstBase.BaseTransform.__init__(self)
        self.first_pts = int(4e18)
        self.pts_offset = None

    def do_transform_ip(self, buffer):
        if self.pts_offset is None:
            self.pts_offset = self.first_pts - buffer.pts
        buffer.pts += self.pts_offset 
        Gst.debug("do_transform_ip: timestamp: %s" % (Gst.TIME_ARGS(buffer.pts)))
        return Gst.FlowReturn.OK


__gstelementfactory__ = ("live_feed_simulator", Gst.Rank.NONE, LiveFeedSimulator)
