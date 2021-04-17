#!/usr/bin/python3
#
# Demonstrates how to run a simple Python Numpy transformation on a video.
#
# Usage:
#   sudo apt install python3-numpy
#   GST_PLUGIN_PATH=$PWD/..:$GST_PLUGIN_PATH \
#   gst-launch-1.0 videotestsrc ! example_python_transform_numpy ! autovideosink
#
# See also https://mathieuduponchelle.github.io/2018-02-15-Python-Elements-2.html
#

import gi
gi.require_version('Gst', '1.0')
gi.require_version('GstBase', '1.0')
gi.require_version('GstVideo', '1.0')
from gi.repository import Gst, GObject, GstBase, GstVideo

import numpy as np


FIXED_CAPS = Gst.Caps.from_string('video/x-raw,format=GRAY8,width=[1,2147483647],height=[1,2147483647]')


class ExampleTransform(GstBase.BaseTransform):
    __gstmetadata__ = (
        'example_python_transform_numpy',
        'Transform',
        'Demonstrates how to run a simple Python Numpy transformation on a video',
        'Claudio Fahey')

    __gsttemplates__ = (Gst.PadTemplate.new("src",
                                           Gst.PadDirection.SRC,
                                           Gst.PadPresence.ALWAYS,
                                           FIXED_CAPS),
                       Gst.PadTemplate.new("sink",
                                           Gst.PadDirection.SINK,
                                           Gst.PadPresence.ALWAYS,
                                           FIXED_CAPS))

    def do_set_caps(self, incaps, outcaps):
        struct = incaps.get_structure(0)
        self.width = struct.get_int("width").value
        self.height = struct.get_int("height").value
        return True

    def do_transform_ip(self, buf):
        try:
            with buf.map(Gst.MapFlags.READ | Gst.MapFlags.WRITE) as info:
                # Create a NumPy ndarray from the memoryview and modify it in place.
                A = np.ndarray(shape=(self.height, self.width), dtype=np.uint8, buffer=info.data)
                Gst.trace("A=%s" % (str(A)))
                A[:] = A * 0.1
                Gst.trace("A=%s" % (str(A)))

                return Gst.FlowReturn.OK
        except Gst.MapError as e:
            Gst.error("Mapping error: %s" % e)
            return Gst.FlowReturn.ERROR


GObject.type_register(ExampleTransform)
__gstelementfactory__ = ("example_python_transform_numpy", Gst.Rank.NONE, ExampleTransform)
