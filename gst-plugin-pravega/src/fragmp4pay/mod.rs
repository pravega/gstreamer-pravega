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

glib::wrapper! {
    pub struct FragMp4Pay(ObjectSubclass<imp::FragMp4Pay>) @extends gst::Element, gst::Object;
}

unsafe impl Send for FragMp4Pay {}
unsafe impl Sync for FragMp4Pay {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        imp::ELEMENT_NAME,
        gst::Rank::None,
        FragMp4Pay::static_type(),
    )
}
