use std::future::Future;
use crate::manager::ManagerResult;

pub trait Routine {
    /// A method that will be called by `Manager` asynchronously when [Manager::run()](Manager::run) is called
    /// 
    /// This can be nested to:
    /// ```
    /// use peachy::prelude::*;
    /// 
    /// struct SomeRoutine;
    /// 
    /// impl Routine for SomeRoutine {
    ///     //        (mut self) is allowed too
    ///     async fn run(self) -> ManagerResult { todo!() }
    /// }
    /// ```
    /// 
    fn run(self) -> impl Future<Output = ManagerResult> + Send + Sync + 'static;
    
    /// The name of the routine.
    /// 
    /// Default is set to "Unnamed"
    /// 
    /// It is needed for `tracing` to identify the routine in logs if you ever need it
    fn name(&self) -> &'static str { "Unnamed" }
}
