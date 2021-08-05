//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use glib::subclass::prelude::*;
use gst::ClockTime;
use gst::prelude::*;
use gst::subclass::prelude::*;
#[allow(unused_imports)]
use gst::{gst_debug, gst_error, gst_warning, gst_info, gst_log, gst_trace};
use once_cell::sync::Lazy;
use pravega_client::client_factory::ClientFactory;
use pravega_client::sync::table::{Table, TableError, Version};
use pravega_client_shared::Scope;
use pravega_video::timestamp::{PravegaTimestamp, NSECOND};
use pravega_video::utils;
use crate::utils::clocktime_to_pravega;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::env;
use std::fmt;
use std::sync::{Arc, Mutex};

pub const ELEMENT_NAME: &str = "pravegatc";
const ELEMENT_CLASS_NAME: &str = "PravegaTC";
const ELEMENT_LONG_NAME: &str = "Pravega Transaction Coordinator";
const ELEMENT_DESCRIPTION: &str = "\
This element can be used in a pipeline with a pravegasrc element to provide failure recovery. \
A pipeline that includes these elements can be restarted after a failure and the pipeline will \
resume from where it left off. \
The current implementation is best-effort which means that some buffers may be processed more than once or never at all. \
The pravegatc element periodically writes the PTS of the current buffer to a Pravega table. \
When the pravegatc element starts, if it finds a PTS in this Pravega table, it sets the start-timestamp property of the pravegasrc element.\
";
const ELEMENT_AUTHOR: &str = "Claudio Fahey <claudio.fahey@dell.com>";
const DEBUG_CATEGORY: &str = ELEMENT_NAME;

const PROPERTY_NAME_TABLE: &str = "table";
const PROPERTY_NAME_CONTROLLER: &str = "controller";
const PROPERTY_NAME_KEYCLOAK_FILE: &str = "keycloak-file";

const DEFAULT_CONTROLLER: &str = "127.0.0.1:9090";
const DEFAULT_RECORD_PERIOD_MSECOND: u64 = 1000;

const PERSISTENT_STATE_TABLE_KEY: &str = "pravegatc.PersistentState";

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq)]
struct PersistentState {
    resume_at_pts: u64,
}

#[derive(Debug)]
struct Settings {
    scope: Option<String>,
    table: Option<String>,
    controller: Option<String>,
    keycloak_file: Option<String>,
    fault_injection_pts: ClockTime,
    record_period: ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scope: None,
            table: None,
            controller: Some(DEFAULT_CONTROLLER.to_owned()),
            keycloak_file: None,
            fault_injection_pts: ClockTime::none(),
            record_period: DEFAULT_RECORD_PERIOD_MSECOND * gst::MSECOND,
        }
    }
}

struct StartedState {
    client_factory: ClientFactory,
    table: Arc<Mutex<Table>>,
    last_recorded_pts: ClockTime,
    // The resume_at_pts that will be written to the persistent state upon end-of-stream.
    final_resume_at_pts: PravegaTimestamp,
}

impl fmt::Debug for StartedState {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "StartedState")
    }
}

enum State {
    Stopped,
    Started {
        state: StartedState,
    }
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

pub struct PravegaTC {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        DEBUG_CATEGORY,
        gst::DebugColorFlags::empty(),
        Some(ELEMENT_LONG_NAME),
    )
});

impl PravegaTC {
    fn set_table(
        &self,
        element: &super::PravegaTC,
        table: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        let (scope, table) = match table {
            Some(table) => {
                let components: Vec<&str> = table.split('/').collect();
                if components.len() != 2 {
                    return Err(glib::Error::new(
                        gst::URIError::BadUri,
                        format!("table parameter '{}' is formatted incorrectly. It must be specified as scope/table.", table).as_str(),
                    ));
                }
                let scope = components[0].to_owned();
                let table = components[1].to_owned();
                (Some(scope), Some(table))
            }
            None => {
                gst_info!(CAT, obj: element, "Resetting `{}` to None", PROPERTY_NAME_TABLE);
                (None, None)
            }
        };
        settings.scope = scope;
        settings.table = table;
        Ok(())
    }

    fn set_controller(
        &self,
        _element: &super::PravegaTC,
        controller: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        settings.controller = controller;
        Ok(())
    }

    fn start(&self, element: &super::PravegaTC) -> Result<(), gst::ErrorMessage> {
        gst_debug!(CAT, obj: element, "start: BEGIN");
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let State::Started { .. } = *state {
                unreachable!("already started");
            }
            let mut settings = self.settings.lock().unwrap();

            // Set fault injection parameters.
            // If the environment variable FAULT_INJECTION_PTS_pravegatc is set to a u64, this element will inject
            // a fault when the PTS reaches this value.
            if let Ok(fault_injection_pts) = str::parse::<u64>(env::var(format!("FAULT_INJECTION_PTS_{}", element.name())).unwrap_or_default().as_str()) {
                settings.fault_injection_pts = fault_injection_pts * gst::NSECOND;
                gst_warning!(CAT, obj: element, "start: fault_injection_pts={:?}", settings.fault_injection_pts);
            }

            let scope_name: String = settings.scope.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Scope is not defined"])
            })?;
            let table_name = settings.table.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Table is not defined"])
            })?;
            let scope = Scope::from(scope_name);
            gst_info!(CAT, obj: element, "start: scope={}, table_name={}", scope, table_name);
            let controller = settings.controller.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Controller is not defined"])
            })?;
            gst_info!(CAT, obj: element, "start: controller={}", controller);
            let keycloak_file = settings.keycloak_file.clone();
            gst_info!(CAT, obj: element, "start: keycloak_file={:?}", keycloak_file);
            let config = utils::create_client_config(controller, keycloak_file).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega client config: {}", error])
            })?;
            gst_debug!(CAT, obj: element, "start: config={:?}", config);
            gst_info!(CAT, obj: element, "start: controller_uri={}:{}", config.controller_uri.domain_name(), config.controller_uri.port());
            gst_info!(CAT, obj: element, "start: is_tls_enabled={}", config.is_tls_enabled);
            gst_info!(CAT, obj: element, "start: is_auth_enabled={}", config.is_auth_enabled);

            let client_factory = ClientFactory::new(config);
            let runtime = client_factory.runtime();

            // Create Pravega table.
            let table = runtime.block_on(client_factory.create_table(scope, table_name));

            // Get last checkpointed state (pts) from Pravega table.
            let persistent_state: Result<Option<(PersistentState, Version)>, TableError> = runtime.block_on(table.get(&PERSISTENT_STATE_TABLE_KEY.to_string()));
            gst_debug!(CAT, obj: element, "start: persistent_state={:?}", persistent_state);
            let persistent_state = persistent_state.unwrap();
            match persistent_state {
                Some((persistent_state, _)) => {
                    let resume_at_pts = PravegaTimestamp::from_nanoseconds(Some(persistent_state.resume_at_pts));
                    gst_info!(CAT, obj: element, "start: Resuming at PTS {:?}", resume_at_pts);
                    let pipeline = element.parent().unwrap().downcast::<gst::Pipeline>().unwrap();
                    let children = pipeline.children();
                    // Find all pravegasrc elements and set start-timestamp property.
                    let mut elements_found = false;
                    for child in children {
                        gst_trace!(CAT, obj: element, "start: child={:?}", child);
                        let child_type_name = child.type_().name();
                        if child_type_name == "PravegaSrc" {
                            gst_debug!(CAT, obj: element, "start: Setting start-timestamp of element {:?}", child.name());
                            child.set_property_from_str("start-mode", "timestamp");
                            child.set_property("start-timestamp", &resume_at_pts.nanoseconds().unwrap()).unwrap();
                            elements_found = true;
                        }
                    }
                    if !elements_found {
                        return Err(gst::error_msg!(gst::ResourceError::Settings, ["PravegaSrc element not found in pipeline"]));
                    }
                },
                None => {
                    gst_info!(CAT, obj: element, "start: No persistent state found.");
                },
            }

            *state = State::Started {
                state: StartedState {
                    client_factory,
                    table: Arc::new(Mutex::new(table)),
                    last_recorded_pts: ClockTime::none(),
                    final_resume_at_pts: PravegaTimestamp::none(),
                },
            };
            gst_info!(CAT, obj: element, "start: Started");
            Ok(())
        })();
        gst_debug!(CAT, obj: element, "start: END: result={:?}", result);
        result
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        element: &super::PravegaTC,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst_trace!(CAT, obj: pad, "sink_chain: Handling buffer {:?}", buffer);

        let (fault_injection_pts, record_period) = {
            let settings = self.settings.lock().unwrap();
            (settings.fault_injection_pts, settings.record_period)
        };

        let mut st = self.state.lock().unwrap();

        let state = match *st {
            State::Started {
                ref mut state,
                ..
            } => state,
            State::Stopped => {
                return Err(gst::FlowError::Error)
            }
        };

        let buffer_pts = buffer.pts();
        let buffer_duration = buffer.duration();

        if fault_injection_pts.is_some() && buffer_pts >= fault_injection_pts {
            gst_error!(CAT, obj: pad, "Injecting fault");
            return Err(gst::FlowError::Error)
        }

        self.srcpad.push(buffer)?;

        if buffer_pts.is_some() {
            // If duration of the buffer is reported as 0, we handle it as a 1 nanosecond duration.
            let duration = cmp::max(1, buffer_duration.nanoseconds().unwrap_or_default());
            let resume_at_pts = clocktime_to_pravega(buffer_pts) + duration * NSECOND;            
            state.final_resume_at_pts = resume_at_pts;

            // Periodically write buffer PTS to persistent state.
            if state.last_recorded_pts.is_none() || state.last_recorded_pts + record_period <= buffer_pts {
                gst_debug!(CAT, obj: element, "sink_chain: writing persistent state to resume at {:?}", resume_at_pts);
                let runtime = state.client_factory.runtime();
                let table = state.table.lock().unwrap();
                let persistent_state = PersistentState {
                    resume_at_pts: resume_at_pts.nanoseconds().unwrap(),
                };
                gst_log!(CAT, obj: element, "sink_chain: writing persistent state {:?}", persistent_state);
                runtime.block_on(table.insert(&PERSISTENT_STATE_TABLE_KEY.to_string(), &persistent_state, -1)).map_err(|error| {
                    gst::element_error!(element, gst::CoreError::Failed, ["Failed to write to Pravega table: {}", error]);
                    gst::FlowError::Error
                })?;
                state.last_recorded_pts = buffer_pts;
            }
        }

        gst_trace!(CAT, obj: element, "sink_chain: END: state={:?}", state);
        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, _pad: &gst::Pad, _element: &super::PravegaTC, event: gst::Event) -> bool {
        self.srcpad.push_event(event)
    }

    fn sink_query(&self, _pad: &gst::Pad, _element: &super::PravegaTC, query: &mut gst::QueryRef) -> bool {
        self.srcpad.peer_query(query)
    }

    fn src_event(&self, _pad: &gst::Pad, _element: &super::PravegaTC, event: gst::Event) -> bool {
        self.sinkpad.push_event(event)
    }

    fn src_query(&self, _pad: &gst::Pad, _element: &super::PravegaTC, query: &mut gst::QueryRef) -> bool {
        self.sinkpad.peer_query(query)
    }

    fn stop(&self, element: &super::PravegaTC) -> Result<(), gst::ErrorMessage> {
        gst_info!(CAT, obj: element, "stop: BEGIN");
        let result = (|| {
            let mut st = self.state.lock().unwrap();
            let state = match *st {
                State::Started {
                    ref mut state,
                    ..
                } => state,
                State::Stopped => {
                    return Ok(())
                }
            };
            if state.final_resume_at_pts.is_some() {
                gst_info!(CAT, obj: element, "stop: writing final persistent state to resume at {:?}", state.final_resume_at_pts);
                let runtime = state.client_factory.runtime();
                let table = state.table.lock().unwrap();
                let persistent_state = PersistentState {
                    resume_at_pts: state.final_resume_at_pts.nanoseconds().unwrap(),
                };
                runtime.block_on(table.insert(&PERSISTENT_STATE_TABLE_KEY.to_string(), &persistent_state, -1)).map_err(|error| {
                    gst::error_msg!(gst::ResourceError::Write, ["Failed to write to Pravega table: {}", error])
                })?;
            }
            *st = State::Stopped;
            Ok(())
        })();
        gst_info!(CAT, obj: element, "stop: END: result={:?}", result);
        result
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PravegaTC {
    const NAME: &'static str = ELEMENT_CLASS_NAME;
    type Type = super::PravegaTC;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        pravega_video::tracing::init();

        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_with_template(&templ, Some("sink"))
            .chain_function(|pad, parent, buffer| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |identity, element| identity.sink_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_query(pad, element, query),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_with_template(&templ, Some("src"))
            .event_function(|pad, parent, event| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_query(pad, element, query),
                )
            })
            .build();

        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
            srcpad,
            sinkpad,
        }
    }
}

impl ObjectImpl for PravegaTC {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpec::new_string(
                PROPERTY_NAME_TABLE,
                "Table",
                "The scope and table name that will be used for storing the persistent state. The format must be 'scope/table'.",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_string(
                PROPERTY_NAME_CONTROLLER,
                "Controller",
                "Pravega controller",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_string(
                PROPERTY_NAME_KEYCLOAK_FILE,
                "Keycloak file",
                "The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.",
                None,
                glib::ParamFlags::WRITABLE,
            ),
        ]});
        PROPERTIES.as_ref()
    }

    // TODO: On error, should set flag that will cause element to fail.
    fn set_property(
        &self,
        obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            PROPERTY_NAME_TABLE => {
                let res = match value.get::<String>() {
                    Ok(table) => self.set_table(&obj, Some(table)),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_TABLE, err);
                }
            },
            PROPERTY_NAME_CONTROLLER => {
                let res = match value.get::<String>() {
                    Ok(controller) => {
                        let controller = if controller.is_empty() {
                            None
                        } else {
                            Some(controller)
                        };
                        self.set_controller(&obj, controller)
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_CONTROLLER, err);
                }
            },
            PROPERTY_NAME_KEYCLOAK_FILE => {
                let res: Result<(), glib::Error> = match value.get::<String>() {
                    Ok(keycloak_file) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.keycloak_file = if keycloak_file.is_empty() {
                            None
                        } else {
                            Some(keycloak_file)
                        };
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_KEYCLOAK_FILE, err);
                }
            },
        _ => unimplemented!(),
        };
    }
}

impl ElementImpl for PravegaTC {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                ELEMENT_LONG_NAME,
                "Generic",
                ELEMENT_DESCRIPTION,
                ELEMENT_AUTHOR,
                )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            vec![src_pad_template, sink_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        element: &Self::Type,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst_trace!(CAT, obj: element, "change_state: Changing state {:?}", transition);
        match transition {
            gst::StateChange::ReadyToPaused => {
                self.start(element).unwrap();
            },
            gst::StateChange::PausedToReady => {
                self.stop(element).unwrap();
            },
            _ => {}
        }
        self.parent_change_state(element, transition)
    }
}
