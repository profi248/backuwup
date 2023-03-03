use crate::db::Database;

use futures_util::StreamExt;
use poem::web::websocket::WebSocket;
use poem::{web::Data, IntoResponse};

#[poem::handler]
pub async fn handler(ws: WebSocket, Data(db): Data<&Database>) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {})
}
