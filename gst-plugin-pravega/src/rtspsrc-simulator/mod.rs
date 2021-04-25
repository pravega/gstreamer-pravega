//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use glib::prelude::*;

mod imp;

// The public Rust wrapper type for our element
glib::wrapper! {
    pub struct RtspSrcSimulator(ObjectSubclass<imp::RtspSrcSimulator>) @extends gst_base::Element, gst::Element, gst::Object;
}

// GStreamer elements need to be thread-safe. For the private implementation this is automatically
// enforced but for the public wrapper type we need to specify this manually.
unsafe impl Send for RtspSrcSimulator {}
unsafe impl Sync for RtspSrcSimulator {}

// Registers the type for our element, and then registers in GStreamer under
// the name "rsrgb2gray" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtspsrcsimulator",
        gst::Rank::None,
        RtspSrcSimulator::static_type(),
    )
}
