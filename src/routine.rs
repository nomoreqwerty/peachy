use std::future::Future;

pub trait Routine: Send + Sync + 'static {
    /// Error type of a routine. Can be any type that implements the [Error](std::error::Error) trait, and can also be individual
    /// for different routines
    ///
    /// For a usage example check [Routine::run()](Routine::run) method
    type Err: Into<anyhow::Error> + Send + Sync + 'static;

    /// The method that [Manager](crate::manager::Manager) will call when calling [Manager::run()](Manager::run)
    ///
    /// **Example:**
    /// ```
    /// use peachy::prelude::*;
    /// use thiserror::Error;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     Manager::new()
    ///         .add_routine(RoutineA)
    ///         .add_routine(RoutineB)
    ///         .run()
    ///         .await
    /// }
    ///
    /// pub struct RoutineA;
    ///
    /// impl Routine for RoutineA {
    ///     type Err = RoutineAError;
    ///
    ///     async fn run(self) -> Result<(), Self::Err> { Ok(()) }
    /// }
    ///
    /// pub struct RoutineB;
    ///
    /// impl Routine for RoutineB {
    ///     type Err = RoutineBError;
    ///
    ///     async fn run(self) -> Result<(), Self::Err> { Ok(()) }
    /// }
    ///
    /// #[derive(Error, Debug)]
    /// pub enum RoutineAError {}
    ///
    /// #[derive(Error, Debug)]
    /// pub enum RoutineBError {}
    /// ```
    fn run(self) -> impl Future<Output = Result<(), Self::Err>> + Send + Sync + 'static;
}
