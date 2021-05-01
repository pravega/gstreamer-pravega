//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use gst::ClockTime;
use pravega_video::timestamp::PravegaTimestamp;

pub fn clocktime_to_pravega(t: ClockTime) -> PravegaTimestamp {
    PravegaTimestamp::from_nanoseconds(t.nanoseconds())
}

pub fn pravega_to_clocktime(t: PravegaTimestamp) -> ClockTime {
    ClockTime(t.nanoseconds())
}
