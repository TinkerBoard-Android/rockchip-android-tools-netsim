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

//! Netsim cxx libraries.

#![allow(dead_code)]

mod http_server;
mod pcap;
mod ranging;
mod version;

use std::pin::Pin;

use cxx::let_cxx_string;
use ffi::CxxServerResponseWriter;
use http_server::server_response::ServerResponseWritable;

use crate::http_server::run_http_server;
use crate::pcap::handlers::handle_pcap_cxx;
use crate::ranging::*;
use crate::version::*;

#[cxx::bridge(namespace = "netsim")]
mod ffi {

    extern "Rust" {

        #[cxx_name = "RunHttpServer"]
        fn run_http_server();

        // Ranging

        #[cxx_name = "DistanceToRssi"]
        fn distance_to_rssi(tx_power: i8, distance: f32) -> i8;

        // Version

        #[cxx_name = "GetVersion"]
        fn get_version() -> String;

        // handle_pcap_cxx translates each argument into an appropriate Rust type

        #[cxx_name = "HandlePcapCxx"]
        fn handle_pcap_cxx(
            responder: Pin<&mut CxxServerResponseWriter>,
            method: String,
            param: String,
            body: String,
        );

        // Packet hub

        #[cxx_name = HandleResponse]
        #[namespace = "netsim::fd"]
        fn handle_response(kind: u32, facade_id: u32, packet: &CxxVector<u8>, packet_type: u32);

    }

    unsafe extern "C++" {
        include!("controller/controller.h");

        #[namespace = "netsim::scene_controller"]
        type AddChipResult;
        fn get_chip_id(self: &AddChipResult) -> u32;
        fn get_device_id(self: &AddChipResult) -> u32;
        fn get_facade_id(self: &AddChipResult) -> u32;

        #[rust_name = "add_chip_cxx"]
        #[namespace = "netsim::scene_controller"]
        fn AddChipCxx(
            guid: &CxxString,
            device_name: &CxxString,
            chip_kind: u32,
            chip_name: &CxxString,
            manufacturer: &CxxString,
            product_name: &CxxString,
        ) -> UniquePtr<AddChipResult>;

        #[rust_name = "remove_chip"]
        #[namespace = "netsim::scene_controller"]
        fn RemoveChip(device_id: u32, chip_id: u32);

        #[rust_name = "get_devices"]
        #[namespace = "netsim::scene_controller"]
        fn GetDevices(
            request: &CxxString,
            response: Pin<&mut CxxString>,
            error_message: Pin<&mut CxxString>,
        ) -> u32;

        #[rust_name = "get_devices_bytes"]
        #[namespace = "netsim::scene_controller"]
        fn GetDevicesBytes(vec: &mut Vec<u8>) -> bool;

        #[rust_name = "patch_device"]
        #[namespace = "netsim::scene_controller"]
        fn PatchDevice(
            request: &CxxString,
            response: Pin<&mut CxxString>,
            error_message: Pin<&mut CxxString>,
        ) -> u32;

        /// A C++ class which can be used to respond to a request.
        include!("frontend/server_response_writable.h");

        #[namespace = "netsim::frontend"]
        type CxxServerResponseWriter;

        #[namespace = "netsim::frontend"]
        fn put_ok_with_length(self: &CxxServerResponseWriter, mime_type: &CxxString, length: u32);

        #[namespace = "netsim::frontend"]
        fn put_chunk(self: &CxxServerResponseWriter, chunk: &[u8]);

        #[namespace = "netsim::frontend"]
        fn put_ok(self: &CxxServerResponseWriter, mime_type: &CxxString, body: &CxxString);

        #[namespace = "netsim::frontend"]
        fn put_error(self: &CxxServerResponseWriter, error_code: u32, error_message: &CxxString);

        include!("packet_hub/packet_hub.h");

        #[namespace = "netsim::packet_hub"]
        fn handle_request_cxx(kind: u32, facade_id: u32, packet: &CxxVector<u8>, packet_type: u32);

    }
}

fn handle_response(_kind: u32, _facade_id: u32, _packet: &cxx::CxxVector<u8>, _packet_type: u32) {}

/// CxxServerResponseWriter is defined in server_response_writable.h
/// Wrapper struct allows the impl to discover the respective C++ methods
struct CxxServerResponseWriterWrapper<'a> {
    writer: Pin<&'a mut CxxServerResponseWriter>,
}

impl ServerResponseWritable for CxxServerResponseWriterWrapper<'_> {
    fn put_ok_with_length(&mut self, mime_type: &str, length: u32) {
        let_cxx_string!(mime_type = mime_type);
        self.writer.put_ok_with_length(&mime_type, length);
    }
    fn put_chunk(&mut self, chunk: &[u8]) {
        self.writer.put_chunk(chunk);
    }
    fn put_ok(&mut self, mime_type: &str, body: &str) {
        let_cxx_string!(mime_type = mime_type);
        let_cxx_string!(body = body);
        self.writer.put_ok(&mime_type, &body);
    }
    fn put_error(&mut self, error_code: u16, error_message: &str) {
        let_cxx_string!(error_message = error_message);
        self.writer.put_error(error_code.into(), &error_message);
    }

    fn put_ok_with_vec(&mut self, _mime_type: &str, _body: Vec<u8>) {
        todo!()
    }
}
