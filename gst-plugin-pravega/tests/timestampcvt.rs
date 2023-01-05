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
use gst::prelude::*;
use gstpravega::utils::{pravega_to_clocktime, now_ntp_clocktime};
use pravega_video::timestamp::PravegaTimestamp;
use std::convert::TryFrom;

fn init() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        gst::init().unwrap();
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

    let first_input_pts = now_ntp_clocktime();
    println!("first_input_pts={}", first_input_pts);
    let first_expected_pts = pravega_to_clocktime(PravegaTimestamp::from_ntp_nanoseconds(first_input_pts.nseconds()));
    println!("first_expected_pts={}", first_expected_pts);

    println!("Simulate start of rtspsrc with PTS starting at 0.");
    push_and_validate(&mut h, 0 * ClockTime::MSECOND, None);
    push_and_validate(&mut h, 1000 * ClockTime::MSECOND, None);
    println!("No PTS.");
    push_and_validate(&mut h, ClockTime::NONE, None);
    println!("Key frame with multiple buffers at same PTS.");
    push_and_validate(&mut h, first_input_pts + 0 * ClockTime::MSECOND, Some(first_expected_pts + 0 * ClockTime::MSECOND));
    push_and_validate(&mut h, first_input_pts + 0 * ClockTime::MSECOND, Some(first_expected_pts + 0 * ClockTime::MSECOND));
    println!("Delta frames.");
    push_and_validate(&mut h, first_input_pts + 50 * ClockTime::MSECOND, Some(first_expected_pts + 50 * ClockTime::MSECOND));
    push_and_validate(&mut h, first_input_pts + 100 * ClockTime::MSECOND, Some(first_expected_pts + 100 * ClockTime::MSECOND));
    println!("Large jump forward.");
    push_and_validate(&mut h, first_input_pts + 1000 * ClockTime::MSECOND, Some(first_expected_pts + 1000 * ClockTime::MSECOND));
    println!("Decreasing PTS.");
    push_and_validate(&mut h, first_input_pts + 500 * ClockTime::MSECOND, Some(first_expected_pts + 1015 * ClockTime::MSECOND));
    push_and_validate(&mut h, first_input_pts + 500 * ClockTime::MSECOND, Some(first_expected_pts + 1015 * ClockTime::MSECOND));
    println!("Next frame but still decreasing.");
    push_and_validate(&mut h, first_input_pts + 550 * ClockTime::MSECOND, Some(first_expected_pts + 1030 * ClockTime::MSECOND));
    push_and_validate(&mut h, first_input_pts + 550 * ClockTime::MSECOND, Some(first_expected_pts + 1030 * ClockTime::MSECOND));
    println!("Back to PTS before decrease.");
    push_and_validate(&mut h, first_input_pts + 1000 * ClockTime::MSECOND, Some(first_expected_pts + 1045 * ClockTime::MSECOND));
    println!("Back to normal.");
    push_and_validate(&mut h, first_input_pts + 1050 * ClockTime::MSECOND, Some(first_expected_pts + 1050 * ClockTime::MSECOND));
    push_and_validate(&mut h, first_input_pts + 1050 * ClockTime::MSECOND, Some(first_expected_pts + 1050 * ClockTime::MSECOND));
    push_and_validate(&mut h, first_input_pts + 1100 * ClockTime::MSECOND, Some(first_expected_pts + 1100 * ClockTime::MSECOND));
    println!("No PTS, part 2.");
    push_and_validate(&mut h, ClockTime::NONE, None);
    println!("Back to normal, part 2.");
    push_and_validate(&mut h, first_input_pts + 1150 * ClockTime::MSECOND, Some(first_expected_pts + 1150 * ClockTime::MSECOND));

    println!("test_timestampcvt: END");
}

#[test]
fn test_timestampcvt_start_at_zero() {
    println!("test_timestampcvt_start_at_zero: BEGIN");
    init();
    let filter = gst::ElementFactory::make("timestampcvt", None).unwrap();
    filter.set_property_from_str("input-timestamp-mode", "start-at-current-time");
    let mut h = gst_check::Harness::with_element(&filter, Some("sink"), Some("src"));
    h.set_src_caps_str("data");
    h.set_sink_caps_str("data");
    h.play();

    println!("Simulate start of rtspsrc with PTS starting at 0.");
    let expected_t0 = pravega_to_clocktime(PravegaTimestamp::now());
    let t0 = push_and_get_pts(&mut h, 0 * ClockTime::MSECOND);
    assert!(t0 > expected_t0 - 60 * ClockTime::SECOND);
    assert!(t0 < expected_t0 + 60 * ClockTime::SECOND);
    push_and_validate(&mut h, 1000 * ClockTime::MSECOND, Some(t0 + 1000 * ClockTime::MSECOND));
    push_and_validate(&mut h, 2000 * ClockTime::MSECOND, Some(t0 + 2000 * ClockTime::MSECOND));

    println!("test_timestampcvt_start_at_zero: END");
}

#[test]
fn test_timestampcvt_start_fixed_time() {
    println!("test_timestampcvt_start_fixed_time: BEGIN");
    init();
    let start_utc = "2001-02-03T04:00:00.000Z".to_owned();
    let start_timestamp = PravegaTimestamp::try_from(Some(start_utc.clone())).unwrap();
    let filter = gst::ElementFactory::make("timestampcvt", None).unwrap();
    filter.set_property_from_str("input-timestamp-mode", "start-at-fixed-time");
    filter.set_property_from_str("start-utc", &start_utc[..]);
    let mut h = gst_check::Harness::with_element(&filter, Some("sink"), Some("src"));
    h.set_src_caps_str("data");
    h.set_sink_caps_str("data");
    h.play();

    let expected_t0 = pravega_to_clocktime(start_timestamp);
    push_and_validate(&mut h, 10000 * ClockTime::MSECOND, Some(expected_t0));
    push_and_validate(&mut h, 10100 * ClockTime::MSECOND, Some(expected_t0 + 100 * ClockTime::MSECOND));

    println!("test_timestampcvt_start_fixed_time: END");
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
            assert_eq!(result.pts().unwrap(), expected_output_pts)
        },
        None => {
            harness.push(buffer).unwrap();
        }
    }
}

fn push_and_get_pts(harness: &mut gst_check::Harness, input_pts: ClockTime) -> ClockTime {
    let buffer = {
        let mut buffer = gst::Buffer::with_size(64).unwrap();
        {
            let buffer_mut = buffer.get_mut().unwrap();
            buffer_mut.set_pts(input_pts);
        }
        buffer
    };
    let result = harness.push_and_pull(buffer).unwrap();
    println!("push_and_get_pts: input_pts={:?}, output={:?}", input_pts, result);
    result.pts()
}
