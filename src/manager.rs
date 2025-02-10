use crate::routine::Routine;
use std::future::Future;
use tokio::task::JoinHandle;

/// **Manager** is responsible for running routines asynchronously
///
/// It creates a pool of asynchronous tasks for each routine and executes them simultaneously when *run()* is awaited
///
/// if any routine falls during execution, the entire program terminates
///
/// Every routine must implement [Routine] trait
///
/// Example:
/// ```
/// use peachy::prelude::*;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     Manager::new()
///         .add_routine(HelloWorldRoutine)
///         .run()
///         .await
/// }
///
/// struct HelloWorldRoutine;
///
/// impl Routine for HelloWorldRoutine {
///     type Err = anyhow::Error;
///
///     async fn run(self) -> Result<(), Self::Err> {
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

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
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
        self.run_routines().await?;
        self.join_routines().await
    }

    async fn run_routines(&mut self) -> ManagerResult {
        self.idle_routines.reverse();

        for idle_routine in self.idle_routines.drain(..) {
            self.running_routines
                .push(tokio::spawn(Box::into_pin(idle_routine.0)));
        }

        Ok(())
    }

    #[inline]
    async fn join_routines(&mut self) -> ManagerResult {
        for worker in self.running_routines.drain(..) {
            worker.await??;
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
        routine: impl Routine<Err = impl Into<anyhow::Error> + Send + Sync + 'static>,
    ) -> Self {
        self.idle_routines.push(IdleRoutine::new(routine)); // .run() is not awaited, so it's not running yet
        self
    }
}

pub type ManagerResult = anyhow::Result<()>;

struct IdleRoutine(Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + Sync + 'static>);

impl IdleRoutine {
    fn new(routine: impl Routine<Err = impl Into<anyhow::Error> + Send + Sync + 'static>) -> Self {
        Self(Box::new(
            async move { routine.run().await.map_err(Into::into) },
        ))
    }
}
