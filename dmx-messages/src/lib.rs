#![no_std]

use postcard::experimental::schema::Schema;
use postcard_rpc::{endpoint, topic};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use ts_rs::TS;

topic!(DmxTopic, DMXMessage, "dmx/data");

endpoint!(DummyEndpoint, (), (), "dummy");

#[derive(Clone, Serialize, Deserialize, Debug, Schema, TS)]
#[ts(export, export_to="../dmx-visualizer/web")]
pub struct DMXMessage {
    #[serde(with = "BigArray")]
    #[ts(type = "Array<number>")]
    pub channels: [u8; 512],
}