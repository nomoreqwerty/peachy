use std::future::Future;
use tokio::task::JoinHandle;
use crate::routines::Routine;

pub type ManagerResult = Result<(), ManagerError>;

/// **Manager** is responsible for running routines asynchronously
/// 
/// Every routine must implement [Routine] trait
///
/// Example:
/// ```
/// use peachy::prelude::*;
/// 
/// #[tokio::main]
/// async fn main() {
///     Manager::new()
///         .add_routine(HelloWorldRoutine)
///         .run()
///         .await
///         .unwrap();
/// }
/// 
/// struct HelloWorldRoutine;
/// 
/// impl Routine for HelloWorldRoutine {
///     async fn run(self) -> ManagerResult {
///         println!("Hello, World!");
///         Ok(())
///     }
/// }
/// ```
/// 
/// This program will execute method [run](Routine::run) of `HelloWorldRoutine` and print `Hello, World!`
pub struct Manager {
    idle_routines: Vec<IdleRoutine>,
    running_routines: Vec<JoinHandle<ManagerResult>>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            idle_routines: vec![],
            running_routines: vec![],
        }
    }

    /// Last method to call on `Manager`. Runs all routines and waits for them to finish
    pub async fn run(mut self) -> ManagerResult {
        self.run_routines().await.map_err(ManagerError::RoutineError)?;
        self.join_routines().await
    }

    async fn run_routines(&mut self) -> anyhow::Result<()> {
        self.idle_routines.reverse();
        for idle_routine in self.idle_routines.drain(..) {
            self.running_routines.push(tokio::spawn(Box::into_pin(idle_routine.handle)));
        }

        Ok(())
    }

    #[inline]
    async fn join_routines(&mut self) -> ManagerResult {
        for worker in self.running_routines.drain(..) {
            worker.await.map_err(ManagerError::RuntimeError)??;
        }
        
        Ok(())
    }
    
    /// Adds routine to [Manager].
    /// 
    /// A routine should implement [Routine] trait.
    ///
    /// For example refer to [Manager] docs
    pub fn add_routine(
        mut self,
        routine: impl Routine,
    ) -> Self {
        self.idle_routines.push(IdleRoutine::new(routine)); // .run() is not awaited, so it's not running yet
        self
    }
}

impl Default for Manager {
    fn default() -> Self { Self::new() }
}

#[derive(thiserror::Error, Debug)]
pub enum ManagerError {
    #[error("{0}")]
    Generic(anyhow::Error),

    #[error("async runtime error: {0}")]
    RuntimeError(#[from] tokio::task::JoinError),

    #[error("routine error: {0}")]
    RoutineError(anyhow::Error),
}

struct IdleRoutine {
    name: &'static str,
    handle: Box<dyn Future<Output = ManagerResult> + Send>,
}

impl IdleRoutine {
    fn new(routine: impl Routine) -> Self {
        Self {
            name: routine.name(),
            handle: Box::new(routine.run()),
        }
    }
}