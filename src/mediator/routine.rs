use crate::manager::ManagerResult;
use crate::mediator::MediatorError;
use crate::routine::Routine;

use dashmap::DashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};
/// **Mediator** is responsible for redirecting messages from routines to other routines.
///
/// Every routine is connectable to **Mediator** with a [Connector](Connector).
///
/// Then using [Connector](Connector) you can send messages from one routine to another.
///
/// Example:
/// ```
/// use peachy::prelude::*;
/// use peachy::mediator::*;
/// use anyhow::Error;
///
/// #[tokio::main]
/// async fn main() -> ManagerResult {
///     let mediator = Mediator::new();
///
///     Manager::new()
///         .add_routine(SenderRoutine { con: mediator.connect(AppRoute::SenderRoutine).await })
///         .add_routine(ReceiverRoutine { con: mediator.connect(AppRoute::ReceiverRoutine).await })
///         .add_routine(mediator) // should be added after all connections
///         .run()
///         .await?;
///
///     Ok(())
/// }
///
/// struct SenderRoutine {
///     con: Connector<AppRoute, AppEvent>
/// }
///
/// impl Routine for SenderRoutine {
///     type Err = NoErr;
///
///     async fn run(mut self) -> Result<(), Self::Err> {
///         self.con.send(AppRoute::ReceiverRoutine, AppEvent::ReceiverRoutine(ReceiverRoutineEvent::Print("Hello, World!".to_string()))).await?;
///         Ok(())
///     }
/// }
///
/// struct ReceiverRoutine {
///     con: Connector<AppRoute, AppEvent>
/// }
///
/// impl Routine for ReceiverRoutine {
///     type Err = Error;
///
///     async fn run(mut self) -> Result<(), Self::Err> {
///         if let AppEvent::ReceiverRoutine(ReceiverRoutineEvent::Print(text)) = self.con.recv().await.unwrap() {
///             println!("{}", text);
///         }
///         Ok(())
///     }
/// }
///
/// #[app_route]
/// enum AppRoute {
///     SenderRoutine,
///     ReceiverRoutine,
/// }
///
/// #[app_event]
/// enum AppEvent {
///     ReceiverRoutine(ReceiverRoutineEvent)
/// }
///
/// #[app_event]
/// enum ReceiverRoutineEvent {
///     Print(String)
/// }
/// ```
pub struct Mediator<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Hash + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    connectors: Arc<DashMap<E, Connector<E, M>>>,
}

impl<E, M> Routine for Mediator<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Hash + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    type Err = MediatorError<E>;

    async fn run(self) -> Result<(), Self::Err> {
        loop {
            for mut connector in self.connectors.iter_mut() {
                match connector.value_mut().rx.try_recv() {
                    Ok(MessagePoint {
                        destination,
                        message,
                    }) => self.redirect(destination, message).await?,
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
        }
    }
}

impl<E, M> Mediator<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Hash + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            connectors: Arc::new(DashMap::new()),
        }
    }

    async fn redirect(&self, to: E, message: Message<E, M>) -> Result<(), MediatorError<E>> {
        let connector = self.connectors.get_mut(&to).unwrap();

        let sourcepoint = message.source.clone();
        let endpoint = to.clone();

        connector
            .tx
            .send(MessagePoint {
                destination: to,
                message,
            })
            .await
            .map_err(|_| MediatorError::ChannelClosed {
                from: sourcepoint,
                to: endpoint,
            })?;

        Ok(())
    }

    pub async fn connect(&self, source: E) -> Connector<E, M> {
        let (mediator_to_connector, connector_from_mediator) = tokio::sync::mpsc::channel(32);
        let (connector_to_mediator, mediator_from_connector) = tokio::sync::mpsc::channel(32);

        self.connectors.insert(
            source.clone(),
            Connector {
                source: source.clone(),
                tx: mediator_to_connector,
                rx: mediator_from_connector,
            },
        );

        Connector {
            source,
            tx: connector_to_mediator,
            rx: connector_from_mediator,
        }
    }
}

impl<E, M> Default for Mediator<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Hash + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct Connector<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    source: E,
    tx: Sender<MessagePoint<E, M>>,
    rx: Receiver<MessagePoint<E, M>>,
}

impl<E, M> Connector<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    #[inline]
    pub async fn send(&mut self, dest: E, msg: M) -> ManagerResult {
        self.tx
            .send(MessagePoint {
                destination: dest.clone(),
                message: Message {
                    source: self.source.clone(),
                    message: msg,
                },
            })
            .await
            .map_err(|_| MediatorError::ChannelClosed {
                from: self.source.clone(),
                to: dest,
            })?;

        Ok(())
    }

    #[inline]
    pub async fn recv(&mut self) -> Option<Message<E, M>> {
        self.rx.recv().await.map(
            |MessagePoint {
                 destination: _,
                 message,
             }| message,
        )
    }

    #[inline]
    pub async fn try_recv(&mut self) -> Result<Message<E, M>, TryRecvError> {
        self.rx.try_recv().map(
            |MessagePoint {
                 destination: _,
                 message,
             }| message,
        )
    }
}

pub struct Message<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    pub source: E,
    pub message: M,
}

struct MessagePoint<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    destination: E,
    message: Message<E, M>,
}
