// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0
use crate::{
    counters::{self, FAILED_LABEL, SUCCEEDED_LABEL},
    logging::*,
    peer_manager::{PeerManagerError, TransportNotification},
    transport::Connection,
};
use anyhow::format_err;
use channel::{self};
use diem_config::network_id::NetworkContext;
use diem_logger::prelude::*;
use diem_time_service::{TimeService, TimeServiceTrait};
use diem_types::{network_address::NetworkAddress, PeerId};
use futures::{
    channel::oneshot,
    future::{BoxFuture, FutureExt},
    io::{AsyncRead, AsyncWrite},
    sink::SinkExt,
    stream::{Fuse, FuturesUnordered, StreamExt},
};
use netcore::transport::{ConnectionOrigin, Transport};
use short_hex_str::AsShortHexStr;
use std::{sync::Arc, time::Instant};

#[derive(Debug)]
pub enum TransportRequest {
    DialPeer(
        PeerId,
        NetworkAddress,
        oneshot::Sender<Result<(), PeerManagerError>>,
    ),
}

/// Responsible for listening for new incoming connections
pub struct TransportHandler<TTransport, TSocket>
where
    TTransport: Transport,
    TSocket: AsyncRead + AsyncWrite,
{
    network_context: Arc<NetworkContext>,
    time_service: TimeService,
    /// [`Transport`] that is used to establish connections
    transport: TTransport,
    listener: Fuse<TTransport::Listener>,
    transport_reqs_rx: channel::Receiver<TransportRequest>,
    transport_notifs_tx: channel::Sender<TransportNotification<TSocket>>,
}

impl<TTransport, TSocket> TransportHandler<TTransport, TSocket>
where
    TTransport: Transport<Output = Connection<TSocket>>,
    TTransport::Listener: 'static,
    TTransport::Inbound: 'static,
    TTransport::Outbound: 'static,
    TSocket: AsyncRead + AsyncWrite + 'static,
{
    pub fn new(
        network_context: Arc<NetworkContext>,
        time_service: TimeService,
        transport: TTransport,
        listen_addr: NetworkAddress,
        transport_reqs_rx: channel::Receiver<TransportRequest>,
        transport_notifs_tx: channel::Sender<TransportNotification<TSocket>>,
    ) -> (Self, NetworkAddress) {
        let (listener, listen_addr) = transport
            .listen_on(listen_addr)
            .expect("Transport listen on fails");
        debug!(
            NetworkSchema::new(&network_context),
            listen_address = listen_addr,
            "{} listening on '{}'",
            network_context,
            listen_addr
        );
        (
            Self {
                network_context,
                time_service,
                transport,
                listener: listener.fuse(),
                transport_reqs_rx,
                transport_notifs_tx,
            },
            listen_addr,
        )
    }

    pub async fn listen(mut self) {
        let mut pending_inbound_connections = FuturesUnordered::new();
        let mut pending_outbound_connections = FuturesUnordered::new();

        debug!(
            NetworkSchema::new(&self.network_context),
            "{} Incoming connections listener Task started", self.network_context
        );

        loop {
            futures::select! {
                dial_request = self.transport_reqs_rx.select_next_some() => {
                    if let Some(fut) = self.dial_peer(dial_request) {
                        pending_outbound_connections.push(fut);
                    }
                },
                incoming_connection = self.listener.select_next_some() => {
                    match incoming_connection {
                        Ok((upgrade, addr)) => {
                            debug!(
                                NetworkSchema::new(&self.network_context)
                                    .network_address(&addr),
                                "{} Incoming connection from {}",
                                self.network_context,
                                addr
                            );

                            counters::pending_connection_upgrades(
                                &self.network_context,
                                ConnectionOrigin::Inbound,
                            )
                            .inc();

                            let start_time = self.time_service.now();
                            pending_inbound_connections.push(upgrade.map(move |out| (out, addr, start_time)));
                        }
                        Err(e) => {
                            info!(
                                NetworkSchema::new(&self.network_context),
                                error = %e,
                                "{} Incoming connection error {}",
                                self.network_context,
                                e
                            );
                        }
                    }
                },
                (upgrade, addr, peer_id, start_time, response_tx) = pending_outbound_connections.select_next_some() => {
                    self.handle_completed_outbound_upgrade(upgrade, addr, peer_id, start_time, response_tx).await;
                },
                (upgrade, addr, start_time) = pending_inbound_connections.select_next_some() => {
                    self.handle_completed_inbound_upgrade(upgrade, addr, start_time).await;
                },
                complete => break,
            }
        }

        warn!(
            NetworkSchema::new(&self.network_context),
            "{} Incoming connections listener Task ended", self.network_context
        );
    }

    fn dial_peer(
        &self,
        dial_peer_request: TransportRequest,
    ) -> Option<
        BoxFuture<
            'static,
            (
                Result<Connection<TSocket>, TTransport::Error>,
                NetworkAddress,
                PeerId,
                Instant,
                oneshot::Sender<Result<(), PeerManagerError>>,
            ),
        >,
    > {
        match dial_peer_request {
            TransportRequest::DialPeer(peer_id, addr, response_tx) => {
                match self.transport.dial(peer_id, addr.clone()) {
                    Ok(upgrade) => {
                        counters::pending_connection_upgrades(
                            &self.network_context,
                            ConnectionOrigin::Outbound,
                        )
                        .inc();

                        let start_time = self.time_service.now();
                        Some(
                            upgrade
                                .map(move |out| (out, addr, peer_id, start_time, response_tx))
                                .boxed(),
                        )
                    }
                    Err(error) => {
                        if let Err(send_err) =
                            response_tx.send(Err(PeerManagerError::from_transport_error(error)))
                        {
                            info!(
                                NetworkSchema::new(&self.network_context).remote_peer(&peer_id),
                                "{} Failed to notify clients of TransportError for Peer {}: {:?}",
                                self.network_context,
                                peer_id.short_str(),
                                send_err
                            );
                        }
                        None
                    }
                }
            }
        }
    }

    async fn handle_completed_outbound_upgrade(
        &mut self,
        upgrade: Result<Connection<TSocket>, TTransport::Error>,
        addr: NetworkAddress,
        peer_id: PeerId,
        start_time: Instant,
        response_tx: oneshot::Sender<Result<(), PeerManagerError>>,
    ) {
        counters::pending_connection_upgrades(&self.network_context, ConnectionOrigin::Outbound)
            .dec();

        let elapsed_time = (self.time_service.now() - start_time).as_secs_f64();
        let upgrade = match upgrade {
            Ok(connection) => {
                let dialed_peer_id = connection.metadata.remote_peer_id;
                if dialed_peer_id == peer_id {
                    Ok(connection)
                } else {
                    Err(PeerManagerError::from_transport_error(format_err!(
                        "Dialed PeerId '{}' differs from expected PeerId '{}'",
                        dialed_peer_id.short_str(),
                        peer_id.short_str()
                    )))
                }
            }
            Err(err) => Err(PeerManagerError::from_transport_error(err)),
        };

        let response = match upgrade {
            Ok(connection) => {
                debug!(
                    NetworkSchema::new(&self.network_context)
                        .connection_metadata(&connection.metadata)
                        .network_address(&addr),
                    "{} Outbound connection '{}' at '{}' successfully upgraded after {:.3} secs",
                    self.network_context,
                    peer_id.short_str(),
                    addr,
                    elapsed_time,
                );

                counters::connection_upgrade_time(
                    &self.network_context,
                    ConnectionOrigin::Outbound,
                    SUCCEEDED_LABEL,
                )
                .observe(elapsed_time);

                // Send the new connection to PeerManager
                let event = TransportNotification::NewConnection(connection);
                self.transport_notifs_tx.send(event).await.unwrap();

                Ok(())
            }
            Err(err) => {
                error!(
                    NetworkSchema::new(&self.network_context)
                        .remote_peer(&peer_id)
                        .network_address(&addr),
                    error = %err,
                    "{} Outbound connection failed for peer {} at {}: {}",
                    self.network_context,
                    peer_id.short_str(),
                    addr,
                    err
                );

                counters::connection_upgrade_time(
                    &self.network_context,
                    ConnectionOrigin::Outbound,
                    FAILED_LABEL,
                )
                .observe(elapsed_time);

                Err(err)
            }
        };

        if let Err(send_err) = response_tx.send(response) {
            warn!(
                NetworkSchema::new(&self.network_context).remote_peer(&peer_id),
                "{} Failed to notify PeerManager of OutboundConnection upgrade result for Peer {}: {:?}",
                self.network_context,
                peer_id.short_str(),
                send_err
            );
        }
    }

    async fn handle_completed_inbound_upgrade(
        &mut self,
        upgrade: Result<Connection<TSocket>, TTransport::Error>,
        addr: NetworkAddress,
        start_time: Instant,
    ) {
        counters::pending_connection_upgrades(&self.network_context, ConnectionOrigin::Inbound)
            .dec();

        let elapsed_time = (self.time_service.now() - start_time).as_secs_f64();
        match upgrade {
            Ok(connection) => {
                debug!(
                    NetworkSchema::new(&self.network_context)
                        .connection_metadata_with_address(&connection.metadata),
                    "{} Inbound connection from {} at {} successfully upgraded after {:.3} secs",
                    self.network_context,
                    connection.metadata.remote_peer_id.short_str(),
                    connection.metadata.addr,
                    elapsed_time,
                );

                counters::connection_upgrade_time(
                    &self.network_context,
                    ConnectionOrigin::Inbound,
                    SUCCEEDED_LABEL,
                )
                .observe(elapsed_time);

                // Send the new connection to PeerManager
                let event = TransportNotification::NewConnection(connection);
                self.transport_notifs_tx.send(event).await.unwrap();
            }
            Err(err) => {
                warn!(
                    NetworkSchema::new(&self.network_context)
                        .network_address(&addr),
                    error = %err,
                    "{} Inbound connection from {} failed to upgrade after {:.3} secs: {}",
                    self.network_context,
                    addr,
                    elapsed_time,
                    err,
                );

                counters::connection_upgrade_time(
                    &self.network_context,
                    ConnectionOrigin::Inbound,
                    FAILED_LABEL,
                )
                .observe(elapsed_time);
            }
        }
    }
}
