
use dmx_messages::DmxTopic;
use postcard_rpc::host_client::HostClient;
use postcard_rpc::standard_icd::{WireError, ERROR_PATH};

#[tokio::main]
async fn main() {
    // for dev in nusb::list_devices().unwrap() {
    //     println!("{:#?}", dev);
    // }

    let client: HostClient<WireError> = HostClient::new_raw_nusb(|d| d.product_string() == Some("dmx-reader"), ERROR_PATH, 8);
    println!("Created client");
    let mut sub = client.subscribe::<DmxTopic>(8).await.unwrap();
    println!("Has sub");
    loop {
        let res = sub.recv().await;
        if let Some(res) = res {
            println!("Received: {:?}", res.channels);
        }
    }
}
