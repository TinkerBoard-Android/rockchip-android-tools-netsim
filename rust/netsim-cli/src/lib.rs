// Copyright 2022 Google LLC
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

//! Command Line Interface for Netsim

mod args;
mod browser;
mod requests;
use args::NetsimArgs;
use clap::Parser;

#[no_mangle]
/// main function for netsim CLI to be called by C wrapper netsim-cl
pub extern "C" fn rust_main() {
    let args = NetsimArgs::parse();
    if matches!(args.command, args::Command::Gui) {
        browser::open("http://localhost:7681/");
        return;
    }
    let grpc_method = args.command.grpc_method();
    let json_string = args.command.request_json();
    let client = frontend_client_cxx::ffi::new_frontend_client();
    let client_result = frontend_client_cxx::send_grpc(client, grpc_method, json_string);
    if client_result.is_ok() {
        println!("Grpc Response Json: {}", client_result.json());
    } else {
        println!("Grpc call error: {}", client_result.err());
    }
}
