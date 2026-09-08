use serde_json::{json, Value};
use std::fmt;

pub(crate) const DEFAULT_REMOTE_HOST: &str = "127.0.0.1:4732";
pub(crate) const DISCONNECTED_MESSAGE: &str = "remote backend disconnected";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RemoteCallError {
    Disconnected,
    DispatchTimeout { seconds: u64 },
    ResponseTimeout { seconds: u64 },
    RpcRejected { message: String },
    Protocol { message: String },
}

impl fmt::Display for RemoteCallError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str(DISCONNECTED_MESSAGE),
            Self::DispatchTimeout { seconds } => write!(
                formatter,
                "remote backend request dispatch timed out after {seconds} seconds"
            ),
            Self::ResponseTimeout { seconds } => write!(
                formatter,
                "remote backend request timed out after {seconds} seconds"
            ),
            Self::RpcRejected { message } | Self::Protocol { message } => {
                formatter.write_str(message)
            }
        }
    }
}

pub(crate) enum IncomingMessage {
    Response {
        id: u64,
        payload: Result<Value, RemoteCallError>,
    },
    Notification {
        method: String,
        params: Value,
    },
}

pub(crate) fn build_request_line(
    id: u64,
    method: &str,
    params: Value,
) -> Result<String, RemoteCallError> {
    let request = json!({
        "id": id,
        "method": method,
        "params": params,
    });
    serde_json::to_string(&request).map_err(|error| RemoteCallError::Protocol {
        message: error.to_string(),
    })
}

pub(crate) fn parse_incoming_line(line: &str) -> Option<IncomingMessage> {
    let message: Value = serde_json::from_str(line).ok()?;

    if let Some(id) = message.get("id").and_then(|value| value.as_u64()) {
        if let Some(error) = message.get("error") {
            let error_message = error
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("remote error")
                .to_string();
            return Some(IncomingMessage::Response {
                id,
                payload: Err(RemoteCallError::RpcRejected {
                    message: error_message,
                }),
            });
        }

        let result = message.get("result").cloned().unwrap_or(Value::Null);
        return Some(IncomingMessage::Response {
            id,
            payload: Ok(result),
        });
    }

    let method = message
        .get("method")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if method.is_empty() {
        return None;
    }
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    Some(IncomingMessage::Notification {
        method: method.to_string(),
        params,
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_incoming_line, IncomingMessage, RemoteCallError};

    #[test]
    fn rpc_rejection_remains_typed_instead_of_becoming_transport_failure() {
        let message =
            parse_incoming_line(r#"{"id":7,"error":{"message":"the same arbitrary text"}}"#)
                .expect("response");
        match message {
            IncomingMessage::Response {
                payload: Err(RemoteCallError::RpcRejected { message }),
                ..
            } => assert_eq!(message, "the same arbitrary text"),
            _ => panic!("expected typed rpc rejection"),
        }
    }

    #[test]
    fn call_error_categories_do_not_depend_on_message_text() {
        let rejected = RemoteCallError::RpcRejected {
            message: "remote backend disconnected".to_string(),
        };
        assert!(matches!(rejected, RemoteCallError::RpcRejected { .. }));
        assert_ne!(rejected, RemoteCallError::Disconnected);
        assert_ne!(
            RemoteCallError::ResponseTimeout { seconds: 300 },
            RemoteCallError::DispatchTimeout { seconds: 15 }
        );
    }
}
