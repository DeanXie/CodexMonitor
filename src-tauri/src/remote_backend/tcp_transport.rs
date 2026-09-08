use tauri::AppHandle;
use tokio::net::TcpStream;

use super::transport::{
    spawn_transport_io, RemoteTransport, RemoteTransportConfig, RemoteTransportError,
    TransportAvailabilityObserver, TransportFuture,
};

pub(crate) struct TcpTransport;

impl RemoteTransport for TcpTransport {
    fn connect(
        &self,
        app: AppHandle,
        config: RemoteTransportConfig,
        availability: TransportAvailabilityObserver,
    ) -> TransportFuture {
        Box::pin(async move {
            let RemoteTransportConfig::Tcp { host, .. } = config;

            let stream =
                TcpStream::connect(host.clone())
                    .await
                    .map_err(|error| RemoteTransportError {
                        message: format!("Failed to connect to remote backend at {host}: {error}"),
                    })?;
            let (reader, writer) = stream.into_split();
            Ok(spawn_transport_io(app, reader, writer, availability))
        })
    }
}
