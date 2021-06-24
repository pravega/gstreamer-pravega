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
use std::convert::TryInto;
use std::sync::Mutex;

pub const ELEMENT_NAME: &str = "pravegatc";
const ELEMENT_CLASS_NAME: &str = "PravegaTC";
const ELEMENT_LONG_NAME: &str = "Pravega Transaction Coordinator";
const ELEMENT_DESCRIPTION: &str = "\
Pravega Transaction Coordinator";
const ELEMENT_AUTHOR: &str = "Claudio Fahey <claudio.fahey@dell.com>";
const DEBUG_CATEGORY: &str = ELEMENT_NAME;

#[derive(Debug)]
struct StartedState {
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
            }
        }
    }
}

pub struct PravegaTC {
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

impl PravegaTC {
    fn sink_chain(
        &self,
        pad: &gst::Pad,
        element: &super::PravegaTC,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst_log!(CAT, obj: pad, "Handling buffer {:?}", buffer);

        let mut state = self.state.lock().unwrap();

        let state = match *state {
            State::Started {
                ref mut state,
                ..
            } => state,
        };

        self.srcpad.push(buffer)
        // gst_trace!(CAT, obj: element, "sink_chain: END: state={:?}", state);
        // Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, _pad: &gst::Pad, _element: &super::PravegaTC, event: gst::Event) -> bool {
        self.srcpad.push_event(event)
    }

    fn sink_query(&self, _pad: &gst::Pad, _element: &super::PravegaTC, query: &mut gst::QueryRef) -> bool {
        self.srcpad.peer_query(query)
    }

    fn src_event(&self, _pad: &gst::Pad, _element: &super::PravegaTC, event: gst::Event) -> bool {
        self.sinkpad.push_event(event)
    }

    fn src_query(&self, _pad: &gst::Pad, _element: &super::PravegaTC, query: &mut gst::QueryRef) -> bool {
        self.sinkpad.peer_query(query)
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PravegaTC {
    const NAME: &'static str = ELEMENT_CLASS_NAME;
    type Type = super::PravegaTC;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_with_template(&templ, Some("sink"))
            .chain_function(|pad, parent, buffer| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |identity, element| identity.sink_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_query(pad, element, query),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_with_template(&templ, Some("src"))
            .event_function(|pad, parent, event| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                PravegaTC::catch_panic_pad_function(
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

impl ObjectImpl for PravegaTC {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }
}

impl ElementImpl for PravegaTC {
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

    // Called whenever the state of the element should be changed. This allows for
    // starting up the element, allocating/deallocating resources or shutting down
    // the element again.
    fn change_state(
        &self,
        element: &Self::Type,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst_trace!(CAT, obj: element, "Changing state {:?}", transition);

        let seek_pos = 1_618_976_450_362_581_066 * gst::NSECOND + 34 * gst::SECOND - 1 * gst::MSECOND;
        // let seek_pos = 0 * gst::SECOND;
        gst_log!(CAT, obj: element, "seek_pos={:?}", seek_pos.nanoseconds());

        let pipeline = element.parent().unwrap().downcast::<gst::Pipeline>().unwrap();
        gst_log!(CAT, obj: element, "parent={:?}", pipeline);
        gst_log!(CAT, obj: element, "parent.name={:?}", pipeline.name());
        let children = pipeline.children();
        gst_log!(CAT, obj: element, "children={:?}", children);
        let src = pipeline.child_by_name("src").unwrap();
        gst_log!(CAT, obj: element, "src={:?}", src);
        src.set_property("start-timestamp", &seek_pos.nanoseconds().unwrap()).unwrap();
        // src.set_property("start-mode", &3).unwrap();

        match transition {
            // gst::StateChange::NullToReady => {}
            gst::StateChange::ReadyToPaused => {
                // let seek_pos = 1616872292866678673 * gst::NSECOND + 30 * gst::SECOND;
                // gst_info!(CAT, obj: element, "Seeking to {:?}", seek_pos);
                // element.seek_simple(
                //     gst::SeekFlags::KEY_UNIT,
                //     seek_pos,
                // ).unwrap();
            }
            gst::StateChange::PausedToPlaying => {
                // let seek_pos = 1616872292866678673 * gst::NSECOND + 30 * gst::SECOND;
                // gst_info!(CAT, obj: element, "Seeking to {:?}", seek_pos);
                // self.sinkpad.get_parent_element().unwrap().seek_simple(gst::SeekFlags::KEY_UNIT, seek_pos).unwrap();
                // element.seek_simple(
                //     gst::SeekFlags::KEY_UNIT,
                //     seek_pos,
                // ).unwrap();
            }
            // gst::StateChange::PlayingToPaused => {}
            // gst::StateChange::PausedToReady => {}
            // gst::StateChange::ReadyToNull => {}
            // gst::StateChange::NullToNull => {}
            // gst::StateChange::ReadyToReady => {}
            // gst::StateChange::PausedToPaused => {}
            // gst::StateChange::PlayingToPlaying => {}
            // gst::StateChange::__Unknown(_) => {}
            _ => {}
        }

        // Call the parent class' implementation of ::change_state()
        self.parent_change_state(element, transition)
    }
}
