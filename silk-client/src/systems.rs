use crate::events::ConnectionRequest;
use crate::state::{ClientState, ConnectionState};
use crate::system_params::{ClientRecv, ClientSend};
use bevy::prelude::*;
use silk_common::bevy_matchbox::{matchbox_socket, prelude::*};
use silk_common::events::SilkClientEvent;
use silk_common::packets::auth::{
    SilkLoginRequestPayload, SilkLoginResponsePayload,
};
use silk_common::SilkSocket;
use silk_common::{ConnectionAddr, PlayerAuthentication};

/// Initialize the socket
pub(crate) fn init_socket(
    mut commands: Commands,
    socket_res: Res<ClientState>,
) {
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
pub(crate) fn reset_socket(
    mut commands: Commands,
    mut state: ResMut<ClientState>,
) {
    commands.close_socket::<MultipleChannels>();
    *state = ClientState::default();
}

/// Reads and handles connection request events
pub(crate) fn connection_event_reader(
    mut cxn_event_reader: EventReader<ConnectionRequest>,
    mut state: ResMut<ClientState>,
    mut next_connection_state: ResMut<NextState<ConnectionState>>,
    current_connection_state: Res<State<ConnectionState>>,
    mut event_wtr: EventWriter<SilkClientEvent>,
) {
    match cxn_event_reader.iter().next() {
        Some(ConnectionRequest::Connect { ip, port, auth }) => {
            if let ConnectionState::Disconnected = current_connection_state.0 {
                let addr = ConnectionAddr::Remote {
                    ip: *ip,
                    port: *port,
                };
                debug!(
                    previous = format!("{current_connection_state:?}"),
                    "set state: connecting"
                );
                state.addr.replace(addr);
                state.auth.replace(auth.to_owned());
                next_connection_state.set(ConnectionState::Establishing);
            }
        }
        Some(ConnectionRequest::Disconnect { reason }) => {
            debug!(
                previous = format!("{current_connection_state:?}"),
                "set state: disconnected"
            );
            next_connection_state.set(ConnectionState::Disconnected);
            event_wtr.send(SilkClientEvent::DisconnectedFromHost {
                reason: reason.to_owned(),
            });
        }
        None => {}
    }
}

/// Translates socket updates into bevy events
pub(crate) fn socket_reader(
    mut state: ResMut<ClientState>,
    mut socket: Option<ResMut<MatchboxSocket<MultipleChannels>>>,
    mut event_wtr: EventWriter<SilkClientEvent>,
    mut login_send: ClientSend<SilkLoginRequestPayload>,
    mut next_connection_state: ResMut<NextState<ConnectionState>>,
) {
    // Create socket events for Silk
    if let Some(socket) = socket.as_mut() {
        // Id changed events
        if let Some(id) = socket.id() {
            if state.id.is_none() {
                state.id.replace(id);
                event_wtr.send(SilkClientEvent::IdAssigned(id));
            }
        }

        // Connection state updates
        for (id, peer_state) in socket.update_peers() {
            match peer_state {
                matchbox_socket::PeerState::Connected => {
                    state.host_id.replace(id);
                    let Some(auth) = state.auth.take() else { panic!("no auth set") };
                    match auth {
                        PlayerAuthentication::Registered {
                            username,
                            password,
                            mfa,
                        } => login_send.reliable_to_host(
                            SilkLoginRequestPayload::RegisteredUser {
                                username,
                                password,
                                mfa,
                            },
                        ),
                        PlayerAuthentication::Guest { username } => login_send
                            .reliable_to_host(SilkLoginRequestPayload::Guest {
                                username,
                            }),
                    }
                }
                matchbox_socket::PeerState::Disconnected => {
                    state.host_id.take();
                    next_connection_state.set(ConnectionState::Disconnected);
                    event_wtr.send(SilkClientEvent::DisconnectedFromHost {
                        reason: Some("Server reset".to_string()),
                    });
                }
            }
        }
    }
}

// Translate login to bevy client events
pub(crate) fn on_login_accepted(
    state: Res<ClientState>,
    mut next_connection_state: ResMut<NextState<ConnectionState>>,
    mut login_read: ClientRecv<SilkLoginResponsePayload>,
    mut event_wtr: EventWriter<SilkClientEvent>,
) {
    for payload in login_read.iter() {
        match payload {
            SilkLoginResponsePayload::Accepted { username } => {
                info!("authenticated as {username}");
                next_connection_state.set(ConnectionState::Connected);
                event_wtr.send(SilkClientEvent::ConnectedToHost {
                    host: state.host_id.unwrap(),
                    username: username.to_owned(),
                });
            }
            SilkLoginResponsePayload::Denied { reason } => {
                error!("login denied, reason: {reason:?}");
                next_connection_state.set(ConnectionState::Disconnected);
                event_wtr.send(SilkClientEvent::DisconnectedFromHost {
                    reason: reason.to_owned(),
                });
            }
        }
    }
}