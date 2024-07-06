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
    fn run(self) -> impl Future<Output = ManagerResult> + Send + 'static;
}
