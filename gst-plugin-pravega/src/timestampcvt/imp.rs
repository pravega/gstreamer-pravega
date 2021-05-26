//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use glib::subclass::prelude::*;
use gst::ClockTime;
use gst::prelude::*;
use gst::subclass::prelude::*;
#[allow(unused_imports)]
use gst::{gst_debug, gst_error, gst_warning, gst_info, gst_log, gst_trace};
use once_cell::sync::Lazy;
use pravega_video::timestamp::{PravegaTimestamp, MSECOND};
use std::sync::Mutex;
use crate::utils::pravega_to_clocktime;

pub const ELEMENT_NAME: &str = "timestampcvt";
const ELEMENT_CLASS_NAME: &str = "TimestampCvt";
const ELEMENT_LONG_NAME: &str = "Convert timestamps";
const ELEMENT_DESCRIPTION: &str = "\
This element converts PTS timestamps for buffers.\
Input buffer timestamps are assumed to be nanoseconds \
since the NTP epoch 1900-01-01 00:00:00 UTC, not including leap seconds. \
Use this for buffers from rtspsrc (ntp-sync=true ntp-time-source=running-time).
Output buffer timestamps are nanoseconds \
since 1970-01-01 00:00:00 TAI International Atomic Time, including leap seconds. \
Use this for pipelines that will eventually write to pravegasink (timestamp-mode=tai). \
This element drops any buffers without PTS. \
Additionally, any PTS values that decrease will have their PTS corrected.";
const ELEMENT_AUTHOR: &str = "Claudio Fahey <claudio.fahey@dell.com>";
const DEBUG_CATEGORY: &str = ELEMENT_NAME;

#[derive(Debug)]
struct StartedState {
    prev_input_pts: ClockTime,
    prev_output_pts: PravegaTimestamp,
}

enum State {
    Started {
        state: StartedState,
    }
}

impl Default for State {
    fn default() -> State {
        State::Started {
            state: StartedState {
                prev_input_pts: ClockTime::none(),
                prev_output_pts: PravegaTimestamp::none(),
            }
        }
    }
}

pub struct TimestampCvt {
    state: Mutex<State>,
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        DEBUG_CATEGORY,
        gst::DebugColorFlags::empty(),
        Some(ELEMENT_LONG_NAME),
    )
});

impl TimestampCvt {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        _element: &super::TimestampCvt,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {

        let mut state = self.state.lock().unwrap();
        let state = match *state {
            State::Started { ref mut state } => state,
        };

        // If PTS is corrected, it will be set to the previous PTS plus this amount.
        let pts_correction_delta = 1 * MSECOND;

        let input_pts = buffer.pts();
        if input_pts.is_some() {
            let output_pts = if state.prev_input_pts.is_some() {
                if state.prev_input_pts == input_pts {
                    // PTS has not changed.
                    state.prev_output_pts
                } else if state.prev_input_pts < input_pts {
                    // PTS has increased.
                    PravegaTimestamp::from_ntp_nanoseconds(input_pts.nseconds())
                } else {
                    // PTS has decreased
                    let time_delta = state.prev_input_pts - input_pts;
                    let corrected_pts = state.prev_output_pts + pts_correction_delta;
                    gst_warning!(CAT, obj: pad, "Input PTS decreased by {} from {} to {}. Correcting PTS to {}.",
                        time_delta, state.prev_input_pts, input_pts, corrected_pts);
                    corrected_pts
                }
            } else {
                // This is our first buffer with a PTS.
                PravegaTimestamp::from_ntp_nanoseconds(input_pts.nseconds())
            };
            let success = if output_pts.is_some() {
                if state.prev_output_pts.is_some() && output_pts < state.prev_output_pts {
                    gst_error!(CAT, obj: pad, "Internal error. prev_output_pts={}, output_pts={}",
                        state.prev_output_pts, output_pts);
                    Err(gst::FlowError::Error)
                } else {
                    state.prev_input_pts = input_pts;
                    state.prev_output_pts = output_pts;
                    let output_pts_clocktime = pravega_to_clocktime(output_pts);
                    let buffer_ref = buffer.make_mut();
                    gst_log!(CAT, obj: pad, "Input PTS {}, Output PTS {:?}", input_pts, output_pts);
                    buffer_ref.set_pts(output_pts_clocktime);
                    self.srcpad.push(buffer)
                }
            } else {
                gst_warning!(CAT, obj: pad, "Dropping buffer because input PTS {} cannot be converted to the range {:?} to {:?}",
                    input_pts, PravegaTimestamp::MIN, PravegaTimestamp::MAX);
                Ok(gst::FlowSuccess::Ok)
            };
            success
        } else {
            gst_warning!(CAT, obj: pad, "Dropping buffer because PTS is none");
            Ok(gst::FlowSuccess::Ok)
        }
    }

    fn sink_event(&self, _pad: &gst::Pad, _element: &super::TimestampCvt, event: gst::Event) -> bool {
        self.srcpad.push_event(event)
    }

    fn sink_query(&self, _pad: &gst::Pad, _element: &super::TimestampCvt, query: &mut gst::QueryRef) -> bool {
        self.srcpad.peer_query(query)
    }

    fn src_event(&self, _pad: &gst::Pad, _element: &super::TimestampCvt, event: gst::Event) -> bool {
        self.sinkpad.push_event(event)
    }

    fn src_query(&self, _pad: &gst::Pad, _element: &super::TimestampCvt, query: &mut gst::QueryRef) -> bool {
        self.sinkpad.peer_query(query)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TimestampCvt {
    const NAME: &'static str = ELEMENT_CLASS_NAME;
    type Type = super::TimestampCvt;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_with_template(&templ, Some("sink"))
            .chain_function(|pad, parent, buffer| {
                TimestampCvt::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |identity, element| identity.sink_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                TimestampCvt::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                TimestampCvt::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_query(pad, element, query),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_with_template(&templ, Some("src"))
        .event_function(|pad, parent, event| {
            TimestampCvt::catch_panic_pad_function(
                parent,
                || false,
                |identity, element| identity.src_event(pad, element, event),
            )
        })
        .query_function(|pad, parent, query| {
            TimestampCvt::catch_panic_pad_function(
                parent,
                || false,
                |identity, element| identity.src_query(pad, element, query),
            )
        })
        .build();

        Self {
            state: Mutex::new(Default::default()),
            srcpad,
            sinkpad,
        }
    }
}

impl ObjectImpl for TimestampCvt {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl ElementImpl for TimestampCvt {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                ELEMENT_LONG_NAME,
                "Generic",
                ELEMENT_DESCRIPTION,
                ELEMENT_AUTHOR,
                )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            vec![src_pad_template, sink_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }
}
