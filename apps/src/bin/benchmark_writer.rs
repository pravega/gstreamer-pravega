use gst::prelude::*;

fn main() {
    // Initialize GStreamer
    gst::init().unwrap();

    let pipeline_description = concat!(
        "   videotestsrc name=src is-live=true do-timestamp=true num-buffers=5",
        " ! video/x-raw,width=160,height=120,framerate=30/1",
        " ! videoconvert",
        " ! x264enc tune=zerolatency",
        " ! mpegtsmux",
        // " ! timestampadd",
        " ! filesink location=without-timestamps5.ts",
        // " ! filesink location=with-timestamps5.ts",
        // " ! pravegasink stream=examples/with-timestamps5",
    );
    let pipeline = gst::parse_launch(pipeline_description).unwrap();

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    // Shutdown pipeline
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
