#![feature(type_alias_impl_trait)]

pub mod manager;
pub mod routine;
pub mod error;
#[cfg(feature = "mediator")]
pub mod mediator;

pub use derive;


pub mod prelude {
    pub use crate::manager::*;
    pub use crate::routine::*;
    pub use crate::derive::*;
}