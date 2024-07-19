#![no_std]

use postcard::experimental::schema::Schema;
use postcard_rpc::topic;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

topic!(DmxTopic, DMXMessage, "dmx/data");

#[derive(Clone, Serialize, Deserialize, Debug, Schema)]
pub struct DMXMessage {
    #[serde(with = "BigArray")]
    pub channels: [u8; 512],
}