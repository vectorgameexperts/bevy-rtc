use bevy::prelude::*;
use silk_common::{bevy_matchbox::prelude::PeerId, events::SocketRecvEvent};
use silk_net::Payload;

#[derive(Default, Debug, Resource)]
pub struct IncomingMessages<M: Payload> {
    pub messages: Vec<(PeerId, M)>,
}

impl<M: Payload> IncomingMessages<M> {
    /// Swaps the event buffers and clears the oldest event buffer. In general,
    /// this should be called once per frame/update.
    pub fn flush(mut incoming: ResMut<Self>) {
        if !incoming.messages.is_empty() {
            trace!("flushing {} messages", incoming.messages.len());
        }
        incoming.messages.clear();
    }

    pub fn read_system(
        mut incoming: ResMut<Self>,
        mut events: EventReader<SocketRecvEvent>,
    ) {
        let mut read = 0;
        for SocketRecvEvent((peer_id, packet)) in events.iter() {
            if let Some(message) = M::from_packet(packet) {
                incoming.messages.push((*peer_id, message));
                read += 1;
            }
        }
        if read > 0 {
            trace!("received {} {} packets", read, M::reflect_name());
        }
    }
}