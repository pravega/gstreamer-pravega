//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

mod counting_reader;
mod counting_writer;
mod numeric;
mod pravegasink;
mod pravegasrc;
mod seekable_byte_stream_writer;
mod seekable_take;
pub mod utils;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    pravegasink::register(plugin)?;
    pravegasrc::register(plugin)?;
    Ok(())
}

gst::plugin_define!(
    pravega,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "unknown",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);
