use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub enum NoErr {}

impl Debug for NoErr {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result { unreachable!() }
}

impl Display for NoErr {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result { unreachable!() }
}

impl Error for NoErr {}