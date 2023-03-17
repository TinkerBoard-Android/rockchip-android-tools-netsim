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

//! The pcap::controller is a controller that manages all pcaps.
//!
//! Provides the API for the frontend to interact with pcaps.

use frontend_proto::common::ChipKind;
use frontend_proto::frontend::GetDevicesResponse;
use frontend_proto::model::{Pcap as ProtoPcap, State};
use protobuf::Message;
use std::collections::HashMap;

use crate::ffi::get_devices_bytes;
use crate::http_server::server_response::ResponseWritable;

use super::handlers::Pcaps;

// Creating a new ProtoPcap with known entries
fn new_with_entry(chip_kind: ChipKind, chip_id: i32, device_name: String) -> ProtoPcap {
    ProtoPcap {
        chip_kind,
        chip_id,
        device_name,
        state: State::OFF,
        valid: true,
        ..Default::default()
    }
}

// Will be deprecated once protobuf v3 is imported
fn state_to_string(state: State) -> &'static str {
    match state {
        State::UNKNOWN => "UNKNOWN",
        State::ON => "ON",
        State::OFF => "OFF",
    }
}

// Will be deprecated once protobuf v3 is imported
fn chip_kind_to_string(chip_kind: ChipKind) -> &'static str {
    match chip_kind {
        ChipKind::UNSPECIFIED => "UNSPECIFIED",
        ChipKind::BLUETOOTH => "BLUETOOTH",
        ChipKind::UWB => "UWB",
        ChipKind::WIFI => "WIFI",
    }
}

// Will be deprecated once protobuf v3 is imported
fn write_to_json_str(key: &str, value: String, out: &mut String) {
    if key == "chip_kind" || key == "device_name" || key == "state" {
        out.push_str(format!(r#""{:}": "{:}","#, key, value).as_str());
    } else {
        out.push_str(format!(r#""{:}": {:},"#, key, value).as_str());
    }
}

// Will be deprecated once protobuf v3 is imported
fn pcap_to_string(proto: &ProtoPcap, out: &mut String) {
    out.push('{');
    write_to_json_str("id", proto.get_id().to_string(), out);
    write_to_json_str("chip_kind", chip_kind_to_string(proto.get_chip_kind()).to_string(), out);
    write_to_json_str("chip_id", proto.get_chip_id().to_string(), out);
    write_to_json_str("device_name", proto.get_device_name().to_string(), out);
    write_to_json_str("state", state_to_string(proto.get_state()).to_string(), out);
    write_to_json_str("size", proto.get_size().to_string(), out);
    write_to_json_str("records", proto.get_records().to_string(), out);
    write_to_json_str("timestamp", proto.get_timestamp().to_string(), out);
    write_to_json_str("valid", proto.get_valid().to_string(), out);
    out.pop();
    out.push_str(r"},");
}

// Perform get_devices and add chips into a pcap hashmap
fn get_pcaps_from_devices() -> HashMap<String, ProtoPcap> {
    // Instantiate pcap hashmap
    let mut new_pcaps = HashMap::<String, ProtoPcap>::new();

    // Perform get_devices_bytes ffi to receive bytes of GetDevicesResponse
    // Print error and return empty hashmap if GetDevicesBytes fails.
    let mut vec = Vec::<u8>::new();
    if !get_devices_bytes(&mut vec) {
        println!("netsim error: GetDevicesBytes failed - returning an empty set of pcaps");
        return new_pcaps;
    }

    // Parse get_devices_response
    let device_response = GetDevicesResponse::parse_from_bytes(&vec).unwrap();

    // Adding to pcap hashmap
    for device in device_response.get_devices() {
        for chip in device.get_chips() {
            let new_pcap = new_with_entry(chip.get_kind(), chip.get_id(), device.get_name().into());
            new_pcaps.insert(Pcaps::get_key(&new_pcap), new_pcap);
        }
    }
    new_pcaps
}

pub fn handle_pcap_list(writer: ResponseWritable, pcaps: &mut Pcaps) {
    // Parse the get_devices_response and add info to ProtoPcap
    let new_pcaps = get_pcaps_from_devices();

    // Merging the active chips (new_pcaps) into the active pcaps
    for pcap in new_pcaps.values() {
        if !pcaps.contains_pcap(pcap) {
            pcaps.insert(pcap.clone());
        }
    }

    // Two cases when device gets disconnected:
    // 1. The device had no pcap, remove completely.
    // 2. The device had pcap, indicate by pcap.set_valid(false)
    enum RemovalIndicator {
        Gone(String),
        Unused(String),
    }

    // Check if the active_pcap entry still exists in the chips.
    let mut removal = Vec::<RemovalIndicator>::new();
    for (key, pcap) in pcaps.iter() {
        if !new_pcaps.contains_key(key) {
            if pcap.get_size() == 0 {
                removal.push(RemovalIndicator::Unused(key.to_string()));
            } else {
                removal.push(RemovalIndicator::Gone(key.to_string()))
            }
        }
    }

    // Now remove/update the pcaps based on the loop above
    for indicator in removal {
        match indicator {
            RemovalIndicator::Unused(key) => pcaps.remove(&key),
            RemovalIndicator::Gone(key) => pcaps.get(&key).unwrap().set_valid(false),
        }
    }

    // Write active pcaps to json string
    let mut out = String::new();
    if pcaps.is_empty() {
        out.push_str(r#"{}"#);
    } else {
        out.push_str(r#"{"pcaps": ["#);
        for pcap in pcaps.values() {
            pcap_to_string(pcap, &mut out)
        }
        out.pop();
        out.push_str(r"]}");
    }
    writer.put_ok("text/json", out.as_str());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_pcap_to_string() {
        let pcap = ProtoPcap::new();
        let mut out = String::new();
        pcap_to_string(&pcap, &mut out);
        let expected = r#"{"id": 0,"chip_kind": "UNSPECIFIED","chip_id": 0,"device_name": "","state": "UNKNOWN","size": 0,"records": 0,"timestamp": 0,"valid": false},"#;
        assert_eq!(out, expected);
    }

    #[test]
    fn test_modified_pcap_to_string() {
        let mut pcap = ProtoPcap::new();
        let mut out = String::new();
        pcap.id = 1;
        pcap.chip_kind = ChipKind::WIFI;
        pcap.device_name = "sample".to_string();
        pcap_to_string(&pcap, &mut out);
        let expected = r#"{"id": 1,"chip_kind": "WIFI","chip_id": 0,"device_name": "sample","state": "UNKNOWN","size": 0,"records": 0,"timestamp": 0,"valid": false},"#;
        assert_eq!(out, expected);
    }
}