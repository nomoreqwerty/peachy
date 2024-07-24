use std::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MediatorError<E>
where 
    E: Debug + PartialEq + Send + Sync + 'static
{
    #[error("channel between {from:?} and {to:?} is closed")]
    ChannelClosed { from: E, to: E }
}