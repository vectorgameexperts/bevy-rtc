use bevy::prelude::*;
use bevy_matchbox::{matchbox_socket, prelude::*};
use events::{SilkSendEvent, SilkSocketEvent};
use silk_common::{ConnectionAddr, SilkSocket};
use std::net::IpAddr;
pub mod events;

/// The socket client abstraction
pub struct SilkClientPlugin;

/// State of the socket
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash, States)]
enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
}

impl Plugin for SilkClientPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SocketState::default())
            .add_state::<ConnectionState>()
            .add_event::<ConnectionRequest>()
            .add_system(event_reader)
            .add_event::<SilkSocketEvent>()
            .add_system(event_writer)
            .add_event::<SilkSendEvent>()
            .add_system(event_sender)
            .add_system(
                init_socket.in_schedule(OnEnter(ConnectionState::Connecting)),
            )
            .add_system(
                reset_socket
                    .in_schedule(OnEnter(ConnectionState::Disconnected)),
            );
    }
}

#[derive(Resource, Default)]
struct SocketState {
    /// The socket address, used for connecting/reconnecting
    pub addr: Option<ConnectionAddr>,
    /// The ID of the host
    pub host_id: Option<PeerId>,
    /// The ID given by the signaling server
    pub id: Option<PeerId>,
}

pub enum ConnectionRequest {
    /// A request to connect to the server through the signaling server; the
    /// ip and port are the signaling server
    Connect { ip: IpAddr, port: u16 },
    /// A request to disconnect from the signaling server; this will also
    /// disconnect from the server
    Disconnect,
}

/// Initialize the socket
fn init_socket(mut commands: Commands, socket_res: Res<SocketState>) {
    if let Some(addr) = &socket_res.addr {
        debug!("address: {addr:?}");

        // Create matchbox socket
        let silk_socket = SilkSocket::new(*addr);
        commands.open_socket(silk_socket.builder());
    } else {
        panic!("state set to connecting without config");
    }
}

/// Reset the internal socket
fn reset_socket(mut commands: Commands, mut state: ResMut<SocketState>) {
    // TODO: This is ugly as shit and should just be commands.close_socket();
    <bevy::prelude::Commands<'_, '_> as bevy_matchbox::CloseSocketExt<
        MultipleChannels,
    >>::close_socket(&mut commands);
    *state = SocketState {
        host_id: None,
        id: None,
        addr: state.addr.take(),
    };
}

/// Reads and handles connection request events
fn event_sender(
    mut socket: Option<ResMut<MatchboxSocket<MultipleChannels>>>,
    state: Res<SocketState>,
    mut silk_event_rdr: EventReader<SilkSendEvent>,
) {
    if let Some(socket) = socket.as_mut() {
        match silk_event_rdr.iter().next() {
            Some(SilkSendEvent::ReliableSend(data)) => {
                let host_id = state.host_id.unwrap();
                socket
                    .channel(SilkSocket::RELIABLE_CHANNEL_INDEX)
                    .unwrap()
                    .send(data.clone(), host_id);
            }
            Some(SilkSendEvent::UnreliableSend(data)) => {
                let host_id = state.host_id.unwrap();
                socket
                    .channel(SilkSocket::UNRELIABLE_CHANNEL_INDEX)
                    .unwrap()
                    .send(data.clone(), host_id);
            }
            None => {}
        }
    }
}

/// Reads and handles connection request events
fn event_reader(
    mut cxn_event_reader: EventReader<ConnectionRequest>,
    commands: Commands,
    mut state: ResMut<SocketState>,
    mut connection_state: ResMut<State<ConnectionState>>,
    mut silk_event_wtr: EventWriter<SilkSocketEvent>,
) {
    match cxn_event_reader.iter().next() {
        Some(ConnectionRequest::Connect { ip, port }) => {
            if let ConnectionState::Disconnected = connection_state.0 {
                let addr = ConnectionAddr::Remote {
                    ip: *ip,
                    port: *port,
                };
                debug!(
                    previous = format!("{connection_state:?}"),
                    "set state: connecting"
                );
                state.addr = Some(addr);
                connection_state.0 = ConnectionState::Connecting;
            }
        }
        Some(ConnectionRequest::Disconnect) => {
            if let ConnectionState::Connected = connection_state.0 {
                debug!(
                    previous = format!("{connection_state:?}"),
                    "set state: disconnected"
                );
                reset_socket(commands, state);
                silk_event_wtr.send(SilkSocketEvent::DisconnectedFromHost);
                connection_state.0 = ConnectionState::Disconnected;
            }
        }
        None => {}
    }
}

/// Translates socket updates into bevy events
fn event_writer(
    mut state: ResMut<SocketState>,
    mut socket: Option<ResMut<MatchboxSocket<MultipleChannels>>>,
    mut event_wtr: EventWriter<SilkSocketEvent>,
    mut connection_state: ResMut<State<ConnectionState>>,
) {
    // Create socket events for Silk
    if let Some(socket) = socket.as_mut() {
        // Id changed events
        if let Some(id) = socket.id() {
            if state.id.is_none() {
                state.id.replace(id);
                event_wtr.send(SilkSocketEvent::IdAssigned(id));
            }
        }

        // Connection state updates
        for (id, peer_state) in socket.update_peers() {
            match peer_state {
                matchbox_socket::PeerState::Connected => {
                    state.host_id.replace(id);
                    connection_state.0 = ConnectionState::Connected;
                    event_wtr.send(SilkSocketEvent::ConnectedToHost(id));
                }
                matchbox_socket::PeerState::Disconnected => {
                    state.host_id.take();
                    connection_state.0 = ConnectionState::Disconnected;
                    event_wtr.send(SilkSocketEvent::DisconnectedFromHost);
                }
            }
        }

        // Collect Unreliable, Reliable messages
        let reliable_msgs = socket
            .channel(SilkSocket::RELIABLE_CHANNEL_INDEX)
            .unwrap()
            .receive();
        let unreliable_msgs = socket
            .channel(SilkSocket::UNRELIABLE_CHANNEL_INDEX)
            .unwrap()
            .receive();
        event_wtr.send_batch(
            reliable_msgs
                .into_iter()
                .chain(unreliable_msgs)
                .map(SilkSocketEvent::Message),
        );
    }
}
