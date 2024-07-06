use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::TryRecvError;
use crate::manager::{ManagerError, ManagerResult};
use crate::routines::Routine;

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
///     async fn run(mut self) -> ManagerResult {
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
///     async fn run(mut self) -> ManagerResult {
///         if let AppEvent::ReceiverRoutine(ReceiverRoutineEvent::Print(text)) = self.con.recv().await.unwrap() {
///             println!("{}", text);
///         }
///         Ok(())
///     }
/// }
///
/// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// enum AppRoute {
///     SenderRoutine,
///     ReceiverRoutine,
/// }
///
/// #[derive(Clone, PartialEq)]
/// enum AppEvent {
///     ReceiverRoutine(ReceiverRoutineEvent)
/// }
///
/// #[derive(Clone, PartialEq)]
/// enum ReceiverRoutineEvent {
///     Print(String)
/// }
/// ```
pub struct Mediator<E, M>
where E: Debug + Clone + Hash + PartialEq + Eq + Send + Sync + 'static,
      M: Clone + PartialEq + Send + Sync + 'static
{
    connectors: Arc<DashMap<E, Connector<E, M>>>,
}

impl<E, M> Routine for Mediator<E, M>
    where E: Debug + Clone + Hash + PartialEq + Eq + Send + Sync + 'static,
          M: Clone + PartialEq + Send + Sync + 'static
{
    async fn run(self) -> ManagerResult {
        loop {
            for mut connector in self.connectors.iter_mut() {
                match connector.value_mut().rx.try_recv() {
                    Ok(MessagePoint { destination: dest, message: msg }) => self.redirect(dest, msg).await?,
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
        }
    }
}

impl<E, M> Mediator<E, M>
    where E: Debug + Clone + Hash + PartialEq + Eq + Send + Sync + 'static,
          M: Clone + PartialEq + Send + Sync + 'static
{
    pub fn new() -> Self {
        Self {
            connectors: Arc::new(DashMap::new()),
        }
    }

    async fn redirect(& self, to: E, message: M) -> ManagerResult {
        let connector = self.connectors.get_mut(&to).unwrap();

        connector
            .tx
            .send(MessagePoint { destination: to, message })
            .await
            .map_err(|error| ManagerError::Generic(error.into()))?;

        Ok(())
    }
    
    pub async fn connect(&self, source: E) -> Connector<E, M> {
        let (mediator_to_connector, connector_from_mediator) = tokio::sync::mpsc::channel(32);
        let (connector_to_mediator, mediator_from_connector) = tokio::sync::mpsc::channel(32);

        self
            .connectors
            .insert(source.clone(), Connector {
                tx: mediator_to_connector,
                rx: mediator_from_connector,
            });

        Connector {
            tx: connector_to_mediator,
            rx: connector_from_mediator,
        }
    }
}

impl<E, M> Default for Mediator<E, M>
    where E: Debug + Clone + Hash + PartialEq + Eq + Send + Sync + 'static,
          M: Clone + PartialEq + Send + Sync + 'static
{
    fn default() -> Self { Self::new() }
}

pub struct Connector<E, M>
    where E: Debug + Send + Sync + 'static,
          M: Send + Sync + 'static
{
    tx: Sender<MessagePoint<E, M>>,
    rx: Receiver<MessagePoint<E, M>>,
}

impl<E, M> Connector<E, M>
    where E: Debug + Send + Sync + 'static,
          M: Send + Sync + 'static
{
    #[inline]
    pub async fn send(&mut self, dest: E, msg: M) -> ManagerResult {
        self
            .tx
            .send(MessagePoint { destination: dest, message: msg })
            .await
            .map_err(|error| ManagerError::Generic(error.into()))?;

        Ok(())
    }

    #[inline]
    pub async fn recv(&mut self) -> Option<M> {
        self.rx.recv().await.map(|MessagePoint { destination: _, message }| message)
    }
}

struct MessagePoint<E, M>
{
    destination: E,
    message: M
}