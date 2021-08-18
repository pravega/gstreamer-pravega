#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

import ctypes
import datetime
import distutils.util
import logging
import gi
gi.require_version("Gst", "1.0")
from gi.repository import GObject, Gst


def add_probe(pipeline, element_name, callback, pad_name="sink", probe_type=Gst.PadProbeType.BUFFER):
    logging.info("add_probe: Adding probe to %s pad of %s" % (pad_name, element_name))
    element = pipeline.get_by_name(element_name)
    if not element:
        raise Exception("Unable to get element %s" % element_name)
    sinkpad = element.get_static_pad(pad_name)
    if not sinkpad:
        raise Exception("Unable to get %s pad of %s" % (pad_name, element_name))
    sinkpad.add_probe(probe_type, callback, 0)


def bus_call(bus, message, loop):
    """Callback for GStreamer bus messages"""
    t = message.type
    if t == Gst.MessageType.EOS:
        logging.info("End-of-stream")
        loop.quit()
    elif t == Gst.MessageType.WARNING:
        err, debug = message.parse_warning()
        logging.warning("%s: %s" % (err, debug))
    elif t == Gst.MessageType.ERROR:
        err, debug = message.parse_error()
        logging.error("%s: %s" % (err, debug))
        loop.quit()
    elif t == Gst.MessageType.ELEMENT:
        details = message.get_structure().to_string()
        logging.info("%s: %s" % (message.src.name, str(details),))
    elif t == Gst.MessageType.PROPERTY_NOTIFY:
        details = message.get_structure().to_string()
        logging.debug("%s: %s" % (message.src.name, str(details),))
    return True


def format_clock_time(ns):
    """Format time in nanoseconds like 01:45:35.975000000"""
    s, ns = divmod(ns, 1000000000)
    m, s = divmod(s, 60)
    h, m = divmod(m, 60)
    return "%u:%02u:%02u.%09u" % (h, m, s, ns)


def glist_iterator(li):
    """Iterator for a pyds.GLib object"""
    while li is not None:
        yield li.data
        li = li.next


def long_to_int(l):
    value = ctypes.c_int(l & 0xffffffff).value
    return value


def make_element(factory_name, element_name):
    """Create a GStreamer element, raising an exception on failure."""
    logging.info("Creating element %s of type %s" % (element_name, factory_name))
    element = Gst.ElementFactory.make(factory_name, element_name)
    if not element:
        raise Exception("Unable to create element %s of type %s" % (element_name, factory_name))
    return element


def resolve_pravega_stream(stream_name, default_scope):
    if stream_name:
        if "/" in stream_name:
            return stream_name
        else:
            if not default_scope:
                raise Exception("Stream %s given without a scope but pravega-scope has not been provided" % stream_name)
            return "%s/%s" % (default_scope, stream_name)
    else:
        return None

def str2bool(v):
    return bool(distutils.util.strtobool(v))


class PravegaTimestamp():
    """This is a Python version of PravegaTimestamp in pravega-video/src/timestamp.rs."""

    # Difference between NTP and Unix epochs.
    # Equals 70 years plus 17 leap days.
    # See [https://stackoverflow.com/a/29138806/5890553].
    UNIX_TO_NTP_SECONDS = (70 * 365 + 17) * 24 * 60 * 60

    # UTC to TAI offset.
    # Below is valid for dates between 2017-01-01 and the next leap second.
    # TODO: Beyond this range, we must use a table that incorporates the leap second schedule.
    # See [https://en.wikipedia.org/wiki/International_Atomic_Time].
    UTC_TO_TAI_SECONDS = 37

    def __init__(self, nanoseconds):
        self._nanoseconds = nanoseconds

    def from_nanoseconds(nanoseconds):
        """Create a PravegaTimestamp from the number of nanoseconds since the TAI epoch 1970-01-01 00:00:00 TAI."""
        return PravegaTimestamp(nanoseconds)

    def nanoseconds(self):
        return self._nanoseconds

    def to_unix_nanoseconds(self):
        return self.nanoseconds() - self.UTC_TO_TAI_SECONDS * 1000*1000*1000

    def to_unix_seconds(self):
        return self.to_unix_nanoseconds() * 1e-9

    def to_iso_8601(self):
        seconds = self.to_unix_seconds()
        return datetime.datetime.fromtimestamp(seconds, datetime.timezone.utc).isoformat()

    def is_valid(self):
        return self.nanoseconds() > 0

    def __repr__(self):
        return "%s (%d ns)" % (self.to_iso_8601(), self.nanoseconds())
