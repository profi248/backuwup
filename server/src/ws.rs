use futures_util::StreamExt;
use poem::{IntoResponse, web::Data};
use poem::web::websocket::WebSocket;
use sqlx::PgPool;

#[poem::handler]
pub async fn handler(
    ws: WebSocket,
    Data(pool): Data<&PgPool>
) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {

    })
}
