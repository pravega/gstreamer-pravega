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
use crate::utils::{pravega_to_clocktime, now_ntp_clocktime};

pub const ELEMENT_NAME: &str = "timestampcvt";
const ELEMENT_CLASS_NAME: &str = "TimestampCvt";
const ELEMENT_LONG_NAME: &str = "Convert timestamps";
const ELEMENT_DESCRIPTION: &str = "\
This element converts PTS timestamps for buffers.\
Use this for pipelines that will eventually write to pravegasink (timestamp-mode=tai). \
This element drops any buffers without PTS. \
Additionally, any PTS values that decrease will have their PTS corrected.";
const ELEMENT_AUTHOR: &str = "Claudio Fahey <claudio.fahey@dell.com>";
const DEBUG_CATEGORY: &str = ELEMENT_NAME;

const PROPERTY_NAME_INPUT_TIMESTAMP_MODE: &str = "input-timestamp-mode";

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::GEnum)]
#[repr(u32)]
#[genum(type_name = "GstInputTimestampMode")]
pub enum InputTimestampMode {
    #[genum(
        name = "Input buffer timestamps are nanoseconds \
                since the NTP epoch 1900-01-01 00:00:00 UTC, not including leap seconds. \
                Use this for buffers from rtspsrc (ntp-sync=true ntp-time-source=running-time) \
                with an RTSP camera that sends RTCP Sender Reports.",
        nick = "ntp"
    )]
    Ntp = 0,
    #[genum(
        name = "Input buffer timestamps do not have a known epoch but relative times are accurate. \
                The offset to TAI time will be calculated as the difference between the system clock and the PTS of the first buffer.
                This element will apply this offset to produce the PTS for each output buffer.",
        nick = "relative"
    )]
    Relative = 1,
}

const DEFAULT_INPUT_TIMESTAMP_MODE: InputTimestampMode = InputTimestampMode::Ntp;

#[derive(Debug)]
struct Settings {
    input_timestamp_mode: InputTimestampMode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            input_timestamp_mode: DEFAULT_INPUT_TIMESTAMP_MODE,
        }
    }
}

#[derive(Debug)]
struct StartedState {
    prev_input_pts: ClockTime,
    prev_output_pts: PravegaTimestamp,
    pts_offset_nanos: Option<i128>,
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
                pts_offset_nanos: None,
            }
        }
    }
}

pub struct TimestampCvt {
    settings: Mutex<Settings>,
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

        let input_timestamp_mode = {
            let settings = self.settings.lock().unwrap();
            settings.input_timestamp_mode
        };

        let mut state = self.state.lock().unwrap();
        let state = match *state {
            State::Started { ref mut state } => state,
        };

        // If input PTS decreases, the output PTS will be set to the previous PTS plus this amount.
        let pts_correction_delta = 1 * MSECOND;

        let input_pts = buffer.pts();
        if input_pts.is_some() {
            let input_nanos = input_pts.nanoseconds().unwrap();
            let corrected_input_pts = match input_timestamp_mode {
                InputTimestampMode::Relative => {
                    if state.pts_offset_nanos.is_none() {
                        let now_ntp = now_ntp_clocktime();
                        state.pts_offset_nanos = Some(now_ntp.nanoseconds().unwrap() as i128 - input_nanos as i128);
                        gst_info!(CAT, obj: pad,
                            "Input buffer PTS timestamps will be adjusted by {} nanoseconds to synchronize with the current system time.",
                            state.pts_offset_nanos.unwrap());
                        }
                    ClockTime::from_nseconds((input_nanos as i128 + state.pts_offset_nanos.unwrap()) as u64)
                },
                _ => input_pts
            };
            let output_pts = if state.prev_input_pts.is_some() {
                if state.prev_input_pts == corrected_input_pts {
                    // PTS has not changed.
                    state.prev_output_pts
                } else {
                    // PTS has changed. Calculate new output PTS.
                    let output_pts = PravegaTimestamp::from_ntp_nanoseconds(corrected_input_pts.nseconds());
                    if state.prev_output_pts < output_pts {
                        // PTS has increased normally.
                        output_pts
                    } else {
                        // Output PTS has decreased.
                        let time_delta = state.prev_output_pts - output_pts;
                        let corrected_pts = state.prev_output_pts + pts_correction_delta;
                        gst_warning!(CAT, obj: pad, "Output PTS would have decreased by {} from {} to {}. Correcting PTS to {}.",
                            time_delta, state.prev_output_pts, output_pts, corrected_pts);
                        corrected_pts
                    }
                }
            } else {
                // This is our first buffer with a PTS.
                PravegaTimestamp::from_ntp_nanoseconds(corrected_input_pts.nseconds())
            };
            let success = if output_pts.is_some() {
                if state.prev_output_pts.is_some() && output_pts < state.prev_output_pts {
                    gst_error!(CAT, obj: pad, "Internal error. prev_output_pts={}, output_pts={}",
                        state.prev_output_pts, output_pts);
                    Err(gst::FlowError::Error)
                } else {
                    state.prev_input_pts = corrected_input_pts;
                    state.prev_output_pts = output_pts;
                    let output_pts_clocktime = pravega_to_clocktime(output_pts);
                    let buffer_ref = buffer.make_mut();
                    gst_log!(CAT, obj: pad, "Input PTS {}, Output PTS {:?}", input_pts, output_pts);
                    buffer_ref.set_pts(output_pts_clocktime);
                    self.srcpad.push(buffer)
                }
            } else {
                // For some RTSP sources, buffers during the first 5 seconds will have PTS near 0.
                // This will be logged as a warning.
                // If this persists for more than 15 seconds, the pipeline will stop with an error.
                gst_warning!(CAT, obj: pad, "Dropping buffer because input PTS {} cannot be converted to the range {:?} to {:?}.",
                    input_pts, PravegaTimestamp::MIN, PravegaTimestamp::MAX);
                if input_pts > 15 * gst::SECOND {
                    gst_error!(CAT, obj: pad,
                        "Input buffers do not have valid PTS timestamps. \
                        If you are using an RTSP source, this may occur if the RTSP source is not sending RTCP Sender Reports. \
                        This can be worked around by setting the property {}={}. \
                        If launched with rtsp-camera-to-pravega.py, then set the environment variable TIMESTAMP_SOURCE=local-clock. \
                        Beware that this will reduce timestamp accuracy.",
                        PROPERTY_NAME_INPUT_TIMESTAMP_MODE, "relative");
                    Err(gst::FlowError::Error)
                    }
                else {
                    Ok(gst::FlowSuccess::Ok)
                }
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
            settings: Mutex::new(Default::default()),
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

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpec::new_enum(
                PROPERTY_NAME_INPUT_TIMESTAMP_MODE,
                "Input timestamp mode",
                "Timestamp mode used by the input",
                InputTimestampMode::static_type(),
                DEFAULT_INPUT_TIMESTAMP_MODE as i32,
                glib::ParamFlags::WRITABLE,
            ),
        ]});
        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            PROPERTY_NAME_INPUT_TIMESTAMP_MODE => {
                let res: Result<(), glib::Error> = match value.get::<InputTimestampMode>() {
                    Ok(timestamp_mode) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.input_timestamp_mode = timestamp_mode;
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_INPUT_TIMESTAMP_MODE, err);
                }
            },
        _ => unimplemented!(),
        };
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
