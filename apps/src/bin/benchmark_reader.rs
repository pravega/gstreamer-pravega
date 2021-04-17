use gst::prelude::*;

fn main() {
    // Initialize GStreamer
    gst::init().unwrap();

    gstpravega::plugin_register_static().unwrap();

    let pipeline_description = concat!(
        "   filesrc location=with-timestamps5.ts",
        // "   pravegasrc stream=examples/with-timestamps1",
        " ! timestampremove",
        " ! fakesink",
    );
    let pipeline = gst::parse_launch(pipeline_description).unwrap();

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.get_bus().unwrap();
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
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
