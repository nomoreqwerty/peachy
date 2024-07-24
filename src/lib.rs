#![feature(type_alias_impl_trait)]

pub mod manager;
pub mod mediator;
pub mod routines;

pub use derive;


pub mod prelude {
    pub use crate::manager::*;
    pub use crate::routines::*;
    pub use crate::derive::*;
}