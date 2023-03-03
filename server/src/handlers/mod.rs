pub(crate) mod register;

use poem::{error::ResponseError, http::StatusCode, Body, Response};

use shared::server_message::ServerMessage;

#[derive(thiserror::Error, Debug)]
#[error("error")]
pub struct ServerResponse(ServerMessage);

impl ResponseError for ServerResponse {
    fn status(&self) -> StatusCode {
        StatusCode::OK
    }

    fn as_response(&self) -> Response {
        let body = Body::from_json(&self.0).expect("Failed to serialize response");
        Response::builder().status(self.status()).body(body)
    }
}
