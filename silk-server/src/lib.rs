use bevy::{prelude::*, time::fixed_timestep::FixedTime};
use bevy_matchbox::matchbox_socket::{PeerId, PeerState};
use bevy_matchbox::multi_channel_socket::*;
use events::{SilkBroadcastEvent, SilkServerEvent};
use silk_common::{ConnectionAddr, SilkSocket};

pub mod events;

/// The socket server abstraction
pub struct SilkServerPlugin {
    /// Whether the signaling server is local or remote
    pub signaler_addr: ConnectionAddr,
    /// Hertz for server tickrate, e.g. 30.0 = 30 times per second
    pub tick_rate: f32,
}

#[derive(Resource)]
struct SocketState {
    /// The socket address, used for connecting/reconnecting
    pub addr: ConnectionAddr,

    /// The ID the signaling server sees us as
    pub id: Option<PeerId>,
}

impl Plugin for SilkServerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SocketState {
            addr: self.signaler_addr,
            id: None,
        })
        .insert_resource(FixedTime::new_from_secs(1.0 / self.tick_rate))
        .configure_sets(
            (
                sets::ReadSocket,
                sets::ProcessIncomingEvents,
                sets::UpdateWorldState,
                sets::ProcessOutgoingEvents,
                sets::WriteSocket,
            )
                .chain(),
        )
        .add_event::<SilkServerEvent>()
        .add_system(
            socket_reader
                .in_base_set(sets::ReadSocket)
                .in_schedule(CoreSchedule::FixedUpdate),
        )
        .add_event::<SilkBroadcastEvent>()
        .add_system(
            broadcast
                .in_base_set(sets::WriteSocket)
                .in_schedule(CoreSchedule::FixedUpdate),
        )
        .add_startup_system(init_socket);
    }
}

/// Initialize the socket
fn init_socket(mut commands: Commands, state: Res<SocketState>) {
    debug!("address: {:?}", state.addr);

    // Create matchbox socket
    let silk_socket = SilkSocket::new(state.addr);
    commands.open_socket(silk_socket.builder());
}

/// Translates socket events into Bevy events
fn socket_reader(
    mut state: ResMut<SocketState>,
    mut socket: ResMut<MatchboxSocket>,
    mut event_wtr: EventWriter<SilkServerEvent>,
) {
    // Id changed events
    if let Some(id) = socket.id() {
        if state.id.is_none() {
            state.id.replace(id);
            event_wtr.send(SilkServerEvent::IdAssigned(id));
        }
    }

    // Check for peer updates
    for (peer, peer_state) in socket.update_peers() {
        match peer_state {
            PeerState::Connected => {
                event_wtr.send(SilkServerEvent::PeerJoined(peer));
            }
            PeerState::Disconnected => {
                event_wtr.send(SilkServerEvent::PeerLeft(peer));
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
            .map(SilkServerEvent::Message),
    );
}

/// Reads and handles server broadcast request events
fn broadcast(
    mut socket: ResMut<MatchboxSocket>,
    mut event_reader: EventReader<SilkBroadcastEvent>,
) {
    while let Some(broadcast) = event_reader.iter().next() {
        match broadcast {
            SilkBroadcastEvent::UnreliableSendAll(packet) => {
                let peers: Vec<PeerId> = socket.connected_peers().collect();
                peers.into_iter().for_each(|peer| {
                    socket
                        .channel(SilkSocket::UNRELIABLE_CHANNEL_INDEX)
                        .unwrap()
                        .send(packet.clone(), peer)
                })
            }
            SilkBroadcastEvent::UnreliableSend((peer, packet)) => socket
                .channel(SilkSocket::UNRELIABLE_CHANNEL_INDEX)
                .unwrap()
                .send(packet.clone(), *peer),
            SilkBroadcastEvent::ReliableSendAll(packet) => {
                let peers: Vec<PeerId> = socket.connected_peers().collect();
                peers.into_iter().for_each(|peer| {
                    socket
                        .channel(SilkSocket::RELIABLE_CHANNEL_INDEX)
                        .unwrap()
                        .send(packet.clone(), peer)
                })
            }
            SilkBroadcastEvent::ReliableSend((peer, packet)) => socket
                .channel(SilkSocket::RELIABLE_CHANNEL_INDEX)
                .unwrap()
                .send(packet.clone(), *peer),
        }
    }
}

pub mod sets {
    use bevy::prelude::*;

    /// Silk plugin reads from silk socket and sends "incoming client
    /// message" events
    #[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
    #[system_set(base)]
    pub struct ReadSocket;

    /// Game receives "incoming client message" events from Silk plugin
    /// and creates "side effects"
    #[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
    #[system_set(base)]
    pub struct ProcessIncomingEvents;

    /// Game updates world state here with the "side effects"
    #[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
    #[system_set(base)]
    pub struct UpdateWorldState;

    /// Game sends broadcast events to Silk plugin (after world state
    /// reacts with "side effects" to create a new world state)
    #[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
    #[system_set(base)]
    pub struct ProcessOutgoingEvents;

    /// Silk plugin reads broadcast events game and sends messages over
    /// the silk socket
    #[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
    #[system_set(base)]
    pub struct WriteSocket;
}
