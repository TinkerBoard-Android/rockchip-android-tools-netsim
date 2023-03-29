// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! PCAP handlers and singleton for HTTP and gRPC server.
//!
//! This module implements a handler for GET, PATCH, LIST pcap
//!
//! /v1/pcap --> handle_pcaps
//! /v1/pcap/{id} --> handle_pcap
//! handle_pcap_cxx calls handle_pcaps or handle_pcap based on the method

use frontend_proto::frontend::GetPcapResponse;
use frontend_proto::model::{Pcap as ProtoPcap, State};
use lazy_static::lazy_static;
use protobuf::Message;
use std::collections::hash_map::{Iter, Values};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::RwLock;

use crate::ffi::CxxServerResponseWriter;
use crate::http_server::http_request::{HttpHeaders, HttpRequest};
use crate::http_server::server_response::ResponseWritable;
use crate::CxxServerResponseWriterWrapper;

use super::managers::{handle_pcap_list, handle_pcap_patch};

// The Pcap resource is a singleton that manages all pcaps
lazy_static! {
    static ref RESOURCE: RwLock<Pcaps> = RwLock::new(Pcaps::new());
}

// Pcaps contains a recent copy of all chips and their ChipKind, chip_id,
// and owning device name. Information for any recent or ongoing captures is
// also stored in the ProtoPcap.
pub type ChipId = i32;
pub type FacadeId = i32;
pub type PcapId = i32;
pub struct Pcaps {
    chip_id_map: HashMap<ChipId, ProtoPcap>,
    facade_id_map: HashMap<FacadeId, ProtoPcap>,
    current_idx: i32,
}

impl Pcaps {
    // The idx starts with 4000 to avoid conflict with other indices that may
    // exist in different resources
    fn new() -> Self {
        Pcaps {
            chip_id_map: HashMap::<ChipId, ProtoPcap>::new(),
            facade_id_map: HashMap::<FacadeId, ProtoPcap>::new(),
            current_idx: 4000,
        }
    }

    pub fn contains_pcap(&self, pcap: &ProtoPcap) -> bool {
        self.chip_id_map.contains_key(&pcap.get_chip_id())
    }

    pub fn get_by_chip_id(&mut self, key: ChipId) -> Option<&mut ProtoPcap> {
        self.chip_id_map.get_mut(&key)
    }

    pub fn get_by_facade_id(&mut self, key: FacadeId) -> Option<&mut ProtoPcap> {
        self.facade_id_map.get_mut(&key)
    }

    pub fn get_by_pcap_id(&mut self, id: PcapId) -> Option<&mut ProtoPcap> {
        self.chip_id_map.iter_mut().map(|(_, pcap)| pcap).find(|pcap| pcap.id == id)
    }

    // TODO: replace with "optional bool" in proto
    pub fn set_state(&mut self, id: PcapId, state: bool) -> bool {
        let capture_state = match state {
            true => State::ON,
            false => State::OFF,
        };
        if let Some(pcap) = self.get_by_pcap_id(id) {
            pcap.set_state(capture_state);
            return true;
        }
        false
    }

    // TODO: invoke GetFacadeId cxx method to obtain facade_id from chip_id
    pub fn insert(&mut self, mut pcap: ProtoPcap) {
        pcap.set_id(self.current_idx);
        self.chip_id_map.insert(pcap.get_chip_id(), pcap);
        self.current_idx += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.chip_id_map.is_empty()
    }

    pub fn iter_chip_id_map(&self) -> Iter<ChipId, ProtoPcap> {
        self.chip_id_map.iter()
    }

    // TODO: remove pcap from facade_id_map
    pub fn remove(&mut self, key: &ChipId) {
        if self.chip_id_map.remove(key).is_none() {
            println!("key does not exist in Pcaps");
        }
    }

    pub fn values(&self) -> Values<ChipId, ProtoPcap> {
        self.chip_id_map.values()
    }
}

/// The Rust pcap handler used directly by Http frontend for LIST, GET, and PATCH
pub fn handle_pcap(request: &HttpRequest, param: &str, writer: ResponseWritable) {
    if request.uri.as_str() == "/v1/pcaps" {
        match request.method.as_str() {
            "GET" => {
                let mut pcaps = RESOURCE.write().unwrap();
                handle_pcap_list(writer, &mut pcaps);
            }
            _ => writer.put_error(404, "Not found."),
        }
    } else {
        match request.method.as_str() {
            "GET" => {
                // TODO: Implement handle_pcap_get in controller.rs
                writer.put_ok_with_length("text/plain", 0);
                let response_bytes = GetPcapResponse::new().write_to_bytes().unwrap();
                writer.put_chunk(&response_bytes);
                writer.put_chunk(&response_bytes);
            }
            "PATCH" => {
                let mut pcaps = RESOURCE.write().unwrap();
                let id = match param.parse::<i32>() {
                    Ok(num) => num,
                    Err(_) => {
                        writer.put_error(404, "Incorrect ID type for pcap, ID should be i32.");
                        return;
                    }
                };
                let body = &request.body;
                let state = String::from_utf8(body.to_vec()).unwrap();
                match state.as_str() {
                    "1" => handle_pcap_patch(writer, &mut pcaps, id, true),
                    "2" => handle_pcap_patch(writer, &mut pcaps, id, false),
                    _ => writer.put_error(404, "Incorrect state for PatchPcap"),
                }
            }
            _ => writer.put_error(404, "Not found."),
        }
    }
}

/// pcap handle cxx for grpc server to call
pub fn handle_pcap_cxx(
    responder: Pin<&mut CxxServerResponseWriter>,
    method: String,
    param: String,
    body: String,
) {
    let mut request = HttpRequest {
        method,
        uri: String::new(),
        headers: HttpHeaders::new(),
        version: "1.1".to_string(),
        body: body.as_bytes().to_vec(),
    };
    if param.is_empty() {
        request.uri = "/v1/pcaps".to_string();
    } else {
        request.uri = format!("/v1/pcaps/{}", param);
    }
    handle_pcap(
        &request,
        param.as_str(),
        &mut CxxServerResponseWriterWrapper { writer: responder },
    );
}
