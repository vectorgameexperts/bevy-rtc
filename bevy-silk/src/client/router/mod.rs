mod receive;
mod send;

use crate::{
    protocol::Payload,
    socket::{common_socket_reader, SilkSocket},
};
use bevy::prelude::*;

pub use receive::IncomingMessages;
pub use send::OutgoingMessages;

pub trait AddNetworkMessageExt {
    fn add_network_message<M: Payload>(&mut self) -> &mut Self;
}

impl AddNetworkMessageExt for App {
    fn add_network_message<M>(&mut self) -> &mut Self
    where
        M: Payload,
    {
        if self.world.contains_resource::<IncomingMessages<M>>()
            || self.world.contains_resource::<OutgoingMessages<M>>()
        {
            panic!("client already contains resource: {}", M::reflect_name());
        }
        self.insert_resource(IncomingMessages::<M> { messages: vec![] })
            .insert_resource(OutgoingMessages::<M> {
                reliable_to_host: vec![],
                unreliable_to_host: vec![],
            })
            .add_systems(
                First,
                (
                    IncomingMessages::<M>::flush,
                    IncomingMessages::<M>::receive_payloads,
                )
                    .chain()
                    .after(common_socket_reader)
                    .run_if(resource_exists::<SilkSocket>()),
            )
            .add_systems(
                Last,
                OutgoingMessages::<M>::send_payloads
                    .run_if(resource_exists::<SilkSocket>()),
            );
        self
    }
}