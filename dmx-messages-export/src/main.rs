use ts_rs::TS;

use dmx_messages::{self, DMXMessage};

const EXPORT_PATH: &str = "src/exports";

fn main() {
    DMXMessage::export_all_to(EXPORT_PATH).unwrap();
}