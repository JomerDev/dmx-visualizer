#![cfg_attr(not(feature = "ts"), no_std)]

use postcard::experimental::schema::Schema;
use postcard_rpc::{endpoint, topic};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

#[cfg(feature = "ts")]
use ts_rs::TS;

topic!(DmxTopic, DMXMessage, "dmx/data");

endpoint!(DummyEndpoint, (), (), "dummy");

#[derive(Clone, Serialize, Deserialize, Debug, Schema)]
#[cfg_attr(feature = "ts", derive(TS))]
pub struct DMXMessage {
    #[serde(with = "BigArray")]
    #[cfg_attr(feature = "ts", ts(type = "Array<number>"))]
    pub channels: [u8; 512],
}