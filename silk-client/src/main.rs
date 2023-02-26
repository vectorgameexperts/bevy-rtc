use futures::{select, FutureExt};
use futures_timer::Delay;
use log::info;
use matchbox_socket::WebRtcSocket;
use std::time::Duration;

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    wasm_bindgen_futures::spawn_local(async_main());
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    "info,matchbox_simple_demo=info,matchbox_socket=info".into()
                }),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_file(false)
                .with_target(false),
        )
        .init();

    async_main().await
}

async fn async_main() {
    info!("Connecting to matchbox");
    let (mut socket, loop_fut) =
        WebRtcSocket::new_unreliable("ws://localhost:3536/Client");

    info!("my id is {:?}", socket.id());

    let loop_fut = loop_fut.fuse();
    futures::pin_mut!(loop_fut);

    let timeout = Delay::new(Duration::from_millis(100));
    futures::pin_mut!(timeout);

    loop {
        for peer in socket.accept_new_connections() {
            info!("Found a peer {:?}", peer);
            let packet = "hello server!".as_bytes().to_vec().into_boxed_slice();
            socket.send(packet, peer);
        }

        for (peer, packet) in socket.receive() {
            info!(
                "Received from {:?}: {:?}",
                peer,
                String::from_utf8_lossy(&packet)
            );
        }
        let disconnected_peers = socket.disconnected_peers();
        if !disconnected_peers.is_empty() {
            info!("Disconnected peers: {:?}", disconnected_peers);
        }

        select! {
            _ = (&mut timeout).fuse() => {
                timeout.reset(Duration::from_millis(100));
            }

            _ = &mut loop_fut => {
                break;
            }
        }
    }

    info!("Done");
}