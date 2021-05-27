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
use gstpravega::utils::pravega_to_clocktime;
use pravega_video::timestamp::{PravegaTimestamp, MSECOND};
use std::convert::TryFrom;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
        gstpravega::plugin_register_static().unwrap();
    });
}

#[test]
fn test_timestampcvt() {
    println!("test_timestampcvt: BEGIN");
    init();
    let filter = gst::ElementFactory::make("timestampcvt", None).unwrap();
    let mut h = gst_check::Harness::with_element(&filter, Some("sink"), Some("src"));
    h.set_src_caps_str("data");
    h.set_sink_caps_str("data");
    h.play();

    // Input PTS 1051896:00:00.000000000, Output PTS 2020-01-01T00:00:00.000000000Z (1577836837000000000 ns, 438288:00:37.000000000)
    let first_input_pts = (120 * 365 + 29) * 24 * 60 * 60 * gst::SECOND;
    println!("first_input_pts={}", first_input_pts);
    let first_expected_pts = pravega_to_clocktime(PravegaTimestamp::from_ntp_nanoseconds(first_input_pts.nseconds()));
    println!("first_expected_pts={}", first_expected_pts);

    println!("Simulate start of rtspsrc with PTS starting at 0.");
    push_and_validate(&mut h, 0 * gst::MSECOND, None);
    push_and_validate(&mut h, 1000 * gst::MSECOND, None);
    println!("No PTS.");
    push_and_validate(&mut h, ClockTime::none(), None);
    println!("Key frame with multiple buffers at same PTS.");
    push_and_validate(&mut h, first_input_pts + 0 * gst::MSECOND, Some(first_expected_pts + 0 * gst::MSECOND));
    push_and_validate(&mut h, first_input_pts + 0 * gst::MSECOND, Some(first_expected_pts + 0 * gst::MSECOND));
    println!("Delta frames.");
    push_and_validate(&mut h, first_input_pts + 50 * gst::MSECOND, Some(first_expected_pts + 50 * gst::MSECOND));
    push_and_validate(&mut h, first_input_pts + 100 * gst::MSECOND, Some(first_expected_pts + 100 * gst::MSECOND));
    println!("Large jump forward.");
    push_and_validate(&mut h, first_input_pts + 1000 * gst::MSECOND, Some(first_expected_pts + 1000 * gst::MSECOND));
    println!("Decreasing PTS.");
    push_and_validate(&mut h, first_input_pts + 500 * gst::MSECOND, Some(first_expected_pts + 1001 * gst::MSECOND));
    push_and_validate(&mut h, first_input_pts + 500 * gst::MSECOND, Some(first_expected_pts + 1001 * gst::MSECOND));
    println!("Next frame but still decreasing.");
    push_and_validate(&mut h, first_input_pts + 550 * gst::MSECOND, Some(first_expected_pts + 1002 * gst::MSECOND));
    push_and_validate(&mut h, first_input_pts + 550 * gst::MSECOND, Some(first_expected_pts + 1002 * gst::MSECOND));
    println!("Back to PTS before decrease.");
    push_and_validate(&mut h, first_input_pts + 1000 * gst::MSECOND, Some(first_expected_pts + 1003 * gst::MSECOND));
    println!("Back to normal.");
    push_and_validate(&mut h, first_input_pts + 1050 * gst::MSECOND, Some(first_expected_pts + 1050 * gst::MSECOND));
    push_and_validate(&mut h, first_input_pts + 1050 * gst::MSECOND, Some(first_expected_pts + 1050 * gst::MSECOND));
    push_and_validate(&mut h, first_input_pts + 1100 * gst::MSECOND, Some(first_expected_pts + 1100 * gst::MSECOND));
    println!("No PTS, part 2.");
    push_and_validate(&mut h, ClockTime::none(), None);
    println!("Back to normal, part 2.");
    push_and_validate(&mut h, first_input_pts + 1150 * gst::MSECOND, Some(first_expected_pts + 1150 * gst::MSECOND));

    println!("test_timestampcvt: END");
}

fn push_and_validate(harness: &mut gst_check::Harness, input_pts: ClockTime, expected_output_pts: Option<ClockTime>) {
    let buffer = {
        let mut buffer = gst::Buffer::with_size(64).unwrap();
        {
            let buffer_mut = buffer.get_mut().unwrap();
            buffer_mut.set_pts(input_pts);
        }
        buffer
    };
    match expected_output_pts {
        Some(expected_output_pts) => {
            let result = harness.push_and_pull(buffer).unwrap();
            println!("push_and_validate: input_pts={:?}, output={:?}", input_pts, result);
            assert_eq!(result.pts(), expected_output_pts)
        },
        None => {
            harness.push(buffer).unwrap();
        }
    }
}
