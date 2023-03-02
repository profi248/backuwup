use crate::db::Database;

use futures_util::StreamExt;
use poem::{IntoResponse, web::Data};
use poem::web::websocket::WebSocket;

#[poem::handler]
pub async fn handler(
    ws: WebSocket,
    Data(db): Data<&Database>
) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {

    })
}
