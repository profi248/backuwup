pub mod backup_request;
pub mod login;
pub mod register;

use poem::{error::ResponseError, http::StatusCode, Body, Response};
use shared::server_message::ServerMessage;

#[derive(thiserror::Error, Debug)]
#[error("error")]
pub struct ErrorWrapper(ServerMessage);

impl ResponseError for ErrorWrapper {
    fn status(&self) -> StatusCode {
        // todo change this
        StatusCode::OK
    }

    fn as_response(&self) -> Response {
        let body = Body::from_json(&self.0).expect("Failed to serialize response");
        Response::builder().status(self.status()).body(body)
    }
}

#[macro_export]
macro_rules! err_msg {
    ( $msg:expr ) => {
        $crate::handlers::ErrorWrapper(
            shared::server_message::ServerMessage::Error(
                shared::server_message::Error::Failure($msg.to_string())
            )
        )
    };
}
