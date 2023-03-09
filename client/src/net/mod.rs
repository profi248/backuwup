pub mod requests;

use futures_util::StreamExt;
use tokio_tungstenite::connect_async;

pub async fn init() {
    // let (mut stream, _response) =
    // connect_async("wss://demo.piesocket.com/v3/channel_123?
    // api_key=VCXCEuvhGcBDP7XhiJJUDvR1e1D3eiVjgZ9VRiaV&notify_self").await
    //     .expect("WS connection failed");
    //
    // loop {
    //     let msg = stream.next().await.unwrap().expect("next message failed");
    //     println!("[ws] {msg}");
    // }
}
