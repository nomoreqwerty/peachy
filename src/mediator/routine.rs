use crate::manager::ManagerResult;
use crate::mediator::MediatorError;
use crate::routine::Routine;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

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
///     let mut mediator = Mediator::new();
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
    receivers: Receivers<E, M>,
    senders: Senders<E, M>,
}

type Receivers<E, M> = Vec<RecvTunnel<E, M>>;
type Senders<E, M> = Arc<Mutex<Vec<SendTunnel<E, M>>>>;

impl<E, M> Routine for Mediator<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Hash + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    type Err = MediatorError<E>;

    async fn run(mut self) -> Result<(), Self::Err> {
        let mut handlers= Vec::with_capacity(self.receivers.len());

        for receiver in self.receivers.drain(..) {
            let redirect_task= tokio::spawn(Self::handle_messages(receiver, self.senders.clone()));
            handlers.push(redirect_task);
        }

        for handle in handlers {
            match handle.await {
                Ok(result) => result?,
                Err(error) => return Err(MediatorError::JoinHandleError(error)),
            }
        }

        Ok(())
    }
}

impl<E, M> Mediator<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Hash + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            receivers: vec![],
            senders: Arc::new(Mutex::new(vec![])),
        }
    }

    async fn redirect(source: E, to: E, message: M, senders: Senders<E, M>) -> Result<(), MediatorError<E>> {
        let senders = senders.lock().await;
        

        let tunnel = senders.iter()
            .find(|&t| t.destination == to)
            .ok_or(MediatorError::TargetUnreachable { target: to.clone() })?;



        tunnel
            .tx
            .send(MessagePoint {
                destination: to.clone(),
                payload: Message { source: source.clone(), message },
            })
            .await
            .map_err(|_| MediatorError::ChannelClosed { from: source, to })?;



        Ok(())
    }

    async fn handle_messages(mut receiver: RecvTunnel<E, M>, send_tunnels: Senders<E, M>) -> Result<(), MediatorError<E>> {
        while let Some(msg_point) = receiver.rx.recv().await {

            Self::redirect(receiver.source.clone(), msg_point.destination, msg_point.payload.message, send_tunnels.clone()).await?;

        }

        Err(MediatorError::TargetUnreachable { target: receiver.source })
    }

    pub async fn connect(&mut self, source: E) -> Connector<E, M> {
        let (mediator_to_connector, connector_from_mediator) = tokio::sync::mpsc::channel(32);
        let (connector_to_mediator, mediator_from_connector) = tokio::sync::mpsc::channel(32);

        self.senders.lock().await.push(
            SendTunnel {
                destination: source.clone(),
                tx: mediator_to_connector,
            },
        );
        
        self.receivers.push(
            RecvTunnel {
                source: source.clone(),
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
                payload: Message {
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
        self.rx.recv().await.map(|MessagePoint { payload: message, .. }| message)
    }

    #[inline]
    pub async fn try_recv(&mut self) -> Result<Message<E, M>, TryRecvError> {
        self.rx.try_recv().map(
            |MessagePoint {
                 destination: _,
                 payload: message,
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

pub(crate) struct MessagePoint<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    pub(crate) destination: E,
    pub(crate) payload: Message<E, M>,
}

pub(crate) struct RecvTunnel<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    pub(crate) source: E,
    pub(crate) rx: Receiver<MessagePoint<E, M>>,
}

pub(crate) struct SendTunnel<E, M>
where
    E: Debug + Clone + PartialEq + Eq + Send + Sync + 'static,
    M: Clone + PartialEq + Send + Sync + 'static,
{
    pub(crate) destination: E,
    pub(crate) tx: Sender<MessagePoint<E, M>>,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use derive::{app_event, app_route};
    use tokio::sync::mpsc::Sender;
    use crate::error::NoErr;
    use crate::manager::Manager;
    use super::*;

    #[app_route]
    enum TestAppRoute {
        A,
        B,
    }

    #[app_event]
    enum TestAppEvent {
        AEvent(AEvent),
        BEvent(BEvent),
    }

    #[app_event]
    enum AEvent {
        Check(i32),
    }

    #[app_event]
    enum BEvent {
        Validate(i32),
    }

    struct ARoutine {
        connector: Connector<TestAppRoute, TestAppEvent>,
    }

    impl Routine for ARoutine {
        type Err = NoErr;

        async fn run(mut self) -> Result<(), Self::Err> {
            self.connector
                .send(TestAppRoute::B, TestAppEvent::BEvent(BEvent::Validate(1)))
                .await
                .unwrap();
            
            self.connector
                .send(TestAppRoute::B, TestAppEvent::BEvent(BEvent::Validate(2)))
                .await
                .unwrap();
            
            self.connector
                .send(TestAppRoute::B, TestAppEvent::BEvent(BEvent::Validate(3)))
                .await
                .unwrap();
            
            self.connector
                .send(TestAppRoute::B, TestAppEvent::BEvent(BEvent::Validate(4)))
                .await
                .unwrap();
            
            Ok(())
        }
    }

    struct BRoutine {
        connector: Connector<TestAppRoute, TestAppEvent>,
        test_tx: Sender<i32>,
    }

    impl Routine for BRoutine {
        type Err = NoErr;

        async fn run(mut self) -> Result<(), Self::Err> {
            while let Some(message) = self.connector.recv().await {
                if let TestAppEvent::BEvent(BEvent::Validate(i)) = message.message {
                    self.test_tx.send(i).await.unwrap();
                }
            }

            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_mediator() {
        let mut mediator = Mediator::new();

        let (tx, rx) = tokio::sync::mpsc::channel(4);
        
        let a_routine = ARoutine {
            connector: mediator.connect(TestAppRoute::A).await,
        };
        let b_routine = BRoutine {
            connector: mediator.connect(TestAppRoute::B).await,
            test_tx: tx
        };

        tokio::spawn(async move {
            Manager::new()
                .add_routine(a_routine)
                .add_routine(b_routine)
                .add_routine(mediator)
                .run()
                .await.unwrap();
        });

        tokio::time::sleep(Duration::from_secs(1)).await;

        assert_eq!(rx.len(), 4);
    }
}