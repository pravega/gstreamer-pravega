// Based on gstreamer-rs/tutorials/src/bin/basic-tutorial-4.rs.
use gst::prelude::*;
use gst::MessageView;

struct CustomData {
    pipeline: gst::Pipeline,  // Our one and only element
    playing: bool,            // Are we in the PLAYING state?
    terminate: bool,          // Should we terminate execution?
    seek_enabled: bool,       // Is seeking enabled for this media?
    seek_done: bool,          // Have we performed the seek already?
    duration: gst::ClockTime, // How long does this media last, in nanoseconds
}

fn main() {
    std::env::set_var("GST_DEBUG", "pravegasrc:6,basesrc:6,mpegtsbase:6,mpegtspacketizer:6");

    // Initialize GStreamer
    gst::init().unwrap();

    // This creates a pipeline by parsing the gst-launch pipeline syntax.
    let pipeline = gst::parse_launch(
        "pravegasrc stream=examples/s9 \
        ! decodebin \
        ! videoconvert \
        ! autovideosink",
    )
    .unwrap();

    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Listen to the bus
    let bus = pipeline.bus().unwrap();
    let mut custom_data = CustomData {
        pipeline,
        playing: false,
        terminate: false,
        seek_enabled: false,
        seek_done: false,
        duration: gst::CLOCK_TIME_NONE,
    };

    while !custom_data.terminate {
        let msg = bus.timed_pop(100 * gst::MSECOND);

        match msg {
            Some(msg) => {
                handle_message(&mut custom_data, &msg);
            }
            None => {
                if custom_data.playing {
                    let position = custom_data
                        .pipeline
                        .query_position::<gst::ClockTime>()
                        .expect("Could not query current position.");

                    // If we didn't know it yet, query the stream duration
                    if custom_data.duration == gst::CLOCK_TIME_NONE {
                        custom_data.duration = custom_data
                            .pipeline
                            .query_duration()
                            .expect("Could not query current duration.")
                    }

                    // Print current position and total duration
                    println!("Position {} {} / {}", position.unwrap_or_default(), position, custom_data.duration);

                    if custom_data.seek_enabled
                        && !custom_data.seek_done
                        && position > 1601791517680733 * gst::USECOND
                    {
                        println!("Performing seek...");
                        custom_data
                            .pipeline
                            .seek_simple(
                                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                                1601791512680733 * gst::USECOND,
                            )
                            .expect("Failed to seek.");
                        // custom_data.seek_done = true;
                    }
                }
            }
        }
    }

    // Shutdown pipeline
    custom_data
        .pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}

fn handle_message(custom_data: &mut CustomData, msg: &gst::Message) {
    match msg.view() {
        MessageView::Error(err) => {
            println!(
                "Error received from element {:?}: {} ({:?})",
                err.src().map(|s| s.path_string()),
                err.error(),
                err.debug()
            );
            custom_data.terminate = true;
        }
        MessageView::Eos(..) => {
            println!("End-Of-Stream reached.");
            custom_data.terminate = true;
        }
        MessageView::DurationChanged(_) => {
            // The duration has changed, mark the current one as invalid
            custom_data.duration = gst::CLOCK_TIME_NONE;
        }
        MessageView::StateChanged(state_changed) => {
            if state_changed
                .src()
                .map(|s| s == custom_data.pipeline)
                .unwrap_or(false)
            {
                let new_state = state_changed.current();
                let old_state = state_changed.old();

                println!(
                    "Pipeline state changed from {:?} to {:?}",
                    old_state, new_state
                );

                custom_data.playing = new_state == gst::State::Playing;
                if custom_data.playing {
                    let mut seeking = gst::query::Seeking::new(gst::Format::Time);
                    if custom_data.pipeline.query(&mut seeking) {
                        let (seekable, start, end) = seeking.result();
                        custom_data.seek_enabled = seekable;
                        if seekable {
                            println!("Seeking is ENABLED from {:?} to {:?}", start, end)
                        } else {
                            println!("Seeking is DISABLED for this stream.")
                        }
                    } else {
                        eprintln!("Seeking query failed.")
                    }
                }
            }
        }
        _ => (),
    }
}
