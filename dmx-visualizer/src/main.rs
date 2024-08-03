use dmx_messages::{DMXMessage, DmxTopic};
use futures_util::StreamExt;
use futures_util::SinkExt;
use futures_util::TryFutureExt;
use postcard_rpc::host_client::HostClient;
use postcard_rpc::standard_icd::{WireError, ERROR_PATH};

use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio::task;
use warp::filters::ws::Message;
use warp::filters::ws::WebSocket;
use warp::Filter;

use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web\\dist\\assets"]
struct Static;

#[tokio::main]
async fn main() {
    let (tx,_) = broadcast::channel::<DMXMessage>(2);

    task::spawn(read_uart(tx.clone()));
    task::spawn(run_warp(tx));

    loop {}
}

async fn read_uart(tx: Sender<DMXMessage>) {
    let client: Result<HostClient<WireError>, String> =
        HostClient::try_new_raw_nusb(|d| d.product_string() == Some("dmx-reader"), ERROR_PATH, 8);
    if let Ok(client) = client {
        println!("Created client");
        let mut sub = client.subscribe::<DmxTopic>(8).await.unwrap();
        println!("Has sub");
        loop {
            let res = sub.recv().await;
            if let Some(res) = res {
                // println!("Has msg");
                let _ = tx.send(res);
            }
        }
    } else {
        println!("{}", client.err().unwrap());
    }
    
}

async fn run_warp(tx: Sender<DMXMessage>) {
    let index = warp::path!().and(warp::fs::file("web\\dist\\index.html"));

    let static_files = warp::path("assets")
        .and(warp::get())
        .and(warp_embed::embed(&Static))
        .boxed();

    let rx = warp::any().map(move || tx.clone().subscribe());

    let ws = warp::path("ws")
        // The `ws()` filter will prepare the Websocket handshake.
        .and(warp::ws())
        .and(rx)
        .map(|ws: warp::ws::Ws, rx: Receiver<DMXMessage>| {
            // This will call our function if the handshake succeeds.
            ws.on_upgrade(move |socket| user_connected(socket, rx))
        });

    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST"])
        .allow_headers(vec!["Content-Type"]);

    // log::info!("Serving myapp on http://localhost:3030/myapp");
    warp::serve(index.or(static_files).or(ws).with(cors))
        .run(([127, 0, 0, 1], 8080))
        .await;
}

async fn user_connected(ws: WebSocket, mut rx: Receiver<DMXMessage>) {
    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, _) = ws.split();

    while let Ok(message) = rx.recv().await {
        user_ws_tx
            .send(Message::text(serde_json::to_string(&message).unwrap()))
            .unwrap_or_else(|e| {
                eprintln!("websocket send error: {}", e);
            })
            .await;
    }

    
}