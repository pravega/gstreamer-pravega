//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use std::sync::Once;
use tracing_subscriber::fmt::format::FmtSpan;

static TRACING_INIT: Once = Once::new();

/// Initialize tracing.
/// If the environment variable PRAVEGA_VIDEO_LOG is set, we output tracing events to stdout.
/// Set it to "trace" to output all events.
/// If PRAVEGA_VIDEO_LOG is unset or set to an empty string, this function does not configure any tracing subscribers.
/// This can be called multiple times.
pub fn init() {
    TRACING_INIT.call_once(|| {
        let filter = std::env::var("PRAVEGA_VIDEO_LOG").unwrap_or_default();
        if !filter.is_empty() {
            // This will fail if there is already a global default tracing subscriber.
            // Any such errors will be ignored.
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_span_events(FmtSpan::CLOSE)
                .try_init();
        }
    })
}
