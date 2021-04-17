// This is similar to gst-launch. It is based on launch.rs in gstreamer-rs.
// It registers the gstpravega plugin and enables logging from the Pravega Rust client.
//
// This is a simplified rust-reimplementation of the gst-launch-<version>
// cli tool. It has no own parameters and simply parses the cli arguments
// as launch syntax.
// When the parsing succeeded, the pipeline is run until the stream ends or an error happens.

use gst::prelude::*;
use log::info;
use std::env;
use std::process;

fn main() {
    env_logger::init();
    info!("launch.rs: BEGIN");

    let pipeline_args = &env::args().collect::<Vec<String>>()[1..];
    let pipeline_args: Vec<_> = pipeline_args.iter().map(String::as_str).collect();

    gst::init().unwrap();

    gstpravega::plugin_register_static().unwrap();

    // Let GStreamer create a pipeline from the parsed launch syntax on the cli.
    // In comparision to the launch_glib_main example, this is using the advanced launch syntax
    // parsing API of GStreamer. The function returns a Result, handing us the pipeline if
    // parsing and creating succeeded, and hands us detailed error information if something
    // went wrong. The error is passed as gst::ParseError. In this example, we separately
    // handle the NoSuchElement error, that GStreamer uses to notify us about elements
    // used within the launch syntax, that are not available (not installed).
    // Especially GUIs should probably handle this case, to tell users that they need to
    // install the corresponding gstreamer plugins.
    let mut context = gst::ParseContext::new();
    let pipeline =
        match gst::parse_launchv_full(&pipeline_args, Some(&mut context), gst::ParseFlags::empty()) {
            Ok(pipeline) => pipeline,
            Err(err) => {
                if let Some(gst::ParseError::NoSuchElement) = err.kind::<gst::ParseError>() {
                    eprintln!("Missing element(s): {:?}", context.get_missing_elements());
                } else {
                    eprintln!("Failed to parse pipeline: {}", err);
                }

                process::exit(-1)
            }
        };
    let bus = pipeline.get_bus().unwrap();

    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                eprintln!(
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

    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
