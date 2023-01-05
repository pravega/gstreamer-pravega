//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// RTSP Source Simulator can be used as part of a pipeline to simulate rtspsrc.
// TODO: This is not yet included in lib.rs because it may not be useful.

use glib::subclass::prelude::*;
use gst::ClockTime;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{debug, error, info, log, trace};

use std::convert::TryInto;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use pravega_video::timestamp::PravegaTimestamp;

const PROPERTY_NAME_FIRST_PTS: &str = "first-pts";

const DEFAULT_FIRST_PTS: u64 = 0;
const DEFAULT_APPLY_OFFSET_AFTER_PTS_MSECOND: u64 = 5000;

#[derive(Debug)]
struct Settings {
    first_pts: u64,
    apply_offset_after_pts: ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            first_pts: DEFAULT_FIRST_PTS,
            apply_offset_after_pts: DEFAULT_APPLY_OFFSET_AFTER_PTS_MSECOND * ClockTime::MSECOND,
        }
    }
}

enum State {
    Started {
        pts_offset: ClockTime,
    },
}

impl Default for State {
    fn default() -> State {
        State::Started {
            pts_offset: ClockTime::NONE,
        }
    }
}

pub struct RtspSrcSimulator {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "rtspsrcsimulator",
        gst::DebugColorFlags::empty(),
        Some("RTSP Source Simulator"),
    )
});

impl RtspSrcSimulator {
    // Called whenever a new buffer is passed to our sink pad. Here buffers should be processed and
    // whenever some output buffer is available have to push it out of the source pad.
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        _element: &super::RtspSrcSimulator,
        mut buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        log!(CAT, obj: pad, "Handling buffer {:?}", buffer);

        let settings = self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        let (pts_offset_setting,) = match *state {
            State::Started {
                ref mut pts_offset,
                ..
            } => (pts_offset,),
        };
        let buffer_pts = buffer.get_pts();
        if buffer_pts.is_some() {
            // It takes around 5 seconds for rtspsrc to receive the RTCP timestamps and set PTS to the NTP timestamp.
            if buffer_pts >= settings.apply_offset_after_pts {
                let pts_offset = match pts_offset_setting.nanoseconds() {
                    Some(_) => *pts_offset_setting,
                    None => {
                        let first_pts = ClockTime::from_nseconds(settings.first_pts);
                        log!(CAT, obj: pad, "first_pts={}", first_pts);
                        log!(CAT, obj: pad, "buffer_pts={}", buffer_pts);
                        let new_pts_offset = first_pts - buffer_pts;
                        log!(CAT, obj: pad, "Got first buffer. PTS offset is {:?}.", new_pts_offset.nanoseconds());
                        *pts_offset_setting = new_pts_offset;
                        new_pts_offset
                    }
                };
                let new_pts = buffer_pts + pts_offset;
                let buffer_ref = buffer.make_mut();
                log!(CAT, obj: pad, "Input PTS {}, Output PTS {}", buffer_pts, new_pts);
                buffer_ref.set_pts(new_pts);
            }
        }

        let timestamp = PravegaTimestamp::from_ntp_nanoseconds(buffer.get_pts().nanoseconds());
        log!(CAT, obj: pad, "Output timestamp {}", timestamp);

        self.srcpad.push(buffer)
    }

    // Called whenever an event arrives on the sink pad. It has to be handled accordingly and in
    // most cases has to be either passed to Pad::event_default() on this pad for default handling,
    // or Pad::push_event() on all pads with the opposite direction for direct forwarding.
    // Here we just pass through all events directly to the source pad.
    //
    // See the documentation of gst::Event and gst::EventRef to see what can be done with
    // events, and especially the gst::EventView type for inspecting events.
    fn sink_event(&self, pad: &gst::Pad, _element: &super::RtspSrcSimulator, event: gst::Event) -> bool {
        log!(CAT, obj: pad, "Handling event {:?}", event);
        self.srcpad.push_event(event)
    }

    // Called whenever a query is sent to the sink pad. It has to be answered if the element can
    // handle it, potentially by forwarding the query first to the peer pads of the pads with the
    // opposite direction, or false has to be returned. Default handling can be achieved with
    // Pad::query_default() on this pad and forwarding with Pad::peer_query() on the pads with the
    // opposite direction.
    // Here we just forward all queries directly to the source pad's peers.
    //
    // See the documentation of gst::Query and gst::QueryRef to see what can be done with
    // queries, and especially the gst::QueryView type for inspecting and modifying queries.
    fn sink_query(
        &self,
        pad: &gst::Pad,
        _element: &super::RtspSrcSimulator,
        query: &mut gst::QueryRef,
    ) -> bool {
        log!(CAT, obj: pad, "Handling query {:?}", query);
        self.srcpad.peer_query(query)
    }

    // Called whenever an event arrives on the source pad. It has to be handled accordingly and in
    // most cases has to be either passed to Pad::event_default() on the same pad for default
    // handling, or Pad::push_event() on all pads with the opposite direction for direct
    // forwarding.
    // Here we just pass through all events directly to the sink pad.
    //
    // See the documentation of gst::Event and gst::EventRef to see what can be done with
    // events, and especially the gst::EventView type for inspecting events.
    fn src_event(&self, pad: &gst::Pad, _element: &super::RtspSrcSimulator, event: gst::Event) -> bool {
        log!(CAT, obj: pad, "Handling event {:?}", event);
        self.sinkpad.push_event(event)
    }

    // Called whenever a query is sent to the source pad. It has to be answered if the element can
    // handle it, potentially by forwarding the query first to the peer pads of the pads with the
    // opposite direction, or false has to be returned. Default handling can be achieved with
    // Pad::query_default() on this pad and forwarding with Pad::peer_query() on the pads with the
    // opposite direction.
    // Here we just forward all queries directly to the sink pad's peers.
    //
    // See the documentation of gst::Query and gst::QueryRef to see what can be done with
    // queries, and especially the gst::QueryView type for inspecting and modifying queries.
    fn src_query(
        &self,
        pad: &gst::Pad,
        _element: &super::RtspSrcSimulator,
        query: &mut gst::QueryRef,
    ) -> bool {
        log!(CAT, obj: pad, "Handling query {:?}", query);
        self.sinkpad.peer_query(query)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for RtspSrcSimulator {
    const NAME: &'static str = "RtspSrcSimulator";
    type Type = super::RtspSrcSimulator;
    type ParentType = gst::Element;

    // Called when a new instance is to be created. We need to return an instance
    // of our struct here and also get the class struct passed in case it's needed
    fn with_class(klass: &Self::Class) -> Self {
        // Create our two pads from the templates that were registered with
        // the class and set all the functions on them.
        //
        // Each function is wrapped in catch_panic_pad_function(), which will
        // - Catch panics from the pad functions and instead of aborting the process
        //   it will simply convert them into an error message and poison the element
        //   instance
        // - Extract our RtspSrcSimulator struct from the object instance and pass it to us
        //
        // Details about what each function is good for is next to each function definition
        let templ = klass.get_pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_with_template(&templ, Some("sink"))
            .chain_function(|pad, parent, buffer| {
                RtspSrcSimulator::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |identity, element| identity.sink_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                RtspSrcSimulator::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                RtspSrcSimulator::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_query(pad, element, query),
                )
            })
            .build();

        let templ = klass.get_pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_with_template(&templ, Some("src"))
            .event_function(|pad, parent, event| {
                RtspSrcSimulator::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                RtspSrcSimulator::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_query(pad, element, query),
                )
            })
            .build();

        // Return an instance of our struct and also include our debug category here.
        // The debug category will be used later whenever we need to put something
        // into the debug logs
        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
            srcpad,
            sinkpad,
        }
    }
}

impl ObjectImpl for RtspSrcSimulator {
    // Called right after construction of a new instance
    fn constructed(&self, obj: &Self::Type) {
        // Call the parent class' ::constructed() implementation first
        self.parent_constructed(obj);

        // Here we actually add the pads we created in RtspSrcSimulator::new() to the
        // element so that GStreamer is aware of their existence.
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpec::new_uint64(
                PROPERTY_NAME_FIRST_PTS,
                "First PTS",
                "The first modified output buffer will have this PTS.",
                0,
                std::u64::MAX,
                DEFAULT_FIRST_PTS,
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
        match pspec.get_name() {
            PROPERTY_NAME_FIRST_PTS => {
                let res: Result<(), glib::Error> = match value.get::<u64>() {
                    Ok(first_pts) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.first_pts = first_pts.unwrap_or_default().try_into().unwrap_or_default();
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_FIRST_PTS, err);
                }
            },
        _ => unimplemented!(),
        };
    }
}

impl ElementImpl for RtspSrcSimulator {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "RTSP Source Simulator",
                "Generic",
                "RTSP Source Simulator can be used as part of a pipeline to simulate rtspsrc. \n
                The element `rtspsrc buffer-mode=none ntp-sync=true ntp-time-source=running-time` \n
                can be simulated with the elements \n
                `videotestsrc is-live=true do-timestamp=true ! rtspsrcsimulator first-pts=3800000000000000000 ! \n
                x264enc ! rtph264pay`.
                The rtspsrcsimulator element modifies the PTS of each buffer.",
                "Claudio Fahey <claudio.fahey@dell.com>",
                )
        });
        Some(&*ELEMENT_METADATA)
    }

    // Create and add pad templates for our sink and source pad. These
    // are later used for actually creating the pads and beforehand
    // already provide information to GStreamer about all possible
    // pads that could exist for this type.
    //
    // Actual instances can create pads based on those pad templates
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            // Our element can accept any possible caps on both pads
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

    // Called whenever the state of the element should be changed. This allows for
    // starting up the element, allocating/deallocating resources or shutting down
    // the element again.
    fn change_state(
        &self,
        element: &Self::Type,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        trace!(CAT, obj: self.instance(), "Changing state {:?}", transition);

        // Call the parent class' implementation of ::change_state()
        self.parent_change_state(element, transition)
    }
}
