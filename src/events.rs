use std::collections::HashMap;

use anomaly::BoxError;
use serde_derive::{Deserialize, Serialize};

use prost::alloc::fmt::Formatter;
use std::fmt;
use tendermint::block::Height;

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct NewBlock {
    pub height: Height,
}

impl NewBlock {
    pub fn new(h: Height) -> NewBlock {
        NewBlock { height: h }
    }
    pub fn set_height(&mut self, height: Height) {
        self.height = height;
    }
    pub fn height(&self) -> Height {
        self.height
    }
}

impl From<NewBlock> for Event {
    fn from(v: NewBlock) -> Self {
        Event::NewBlock(v)
    }
}

impl fmt::Display for NewBlock {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "height: {}", self.height)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Event {
    NewBlock(NewBlock),
    Empty(String),      // Special event, signifying empty response
    ChainError(String), // Special event, signifying an error on CheckTx or DeliverTx
}

/// For use in debug messages
pub struct PrettyEvents<'a>(pub &'a [Event]);
impl<'a> fmt::Display for PrettyEvents<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "events:")?;
        for v in self.0 {
            writeln!(f, "\t{}", v)?;
        }
        Ok(())
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Event::NewBlock(ev) => write!(f, "NewBlockEv({})", ev),
            Event::Empty(ev) => write!(f, "EmptyEv({})", ev),
            Event::ChainError(ev) => write!(f, "ChainErrorEv({})", ev),
        }
    }
}

impl Event {
    pub fn to_json(&self) -> String {
        match serde_json::to_string(self) {
            Ok(value) => value,
            Err(_) => format!("{:?}", self), // Fallback to debug printing
        }
    }

    pub fn height(&self) -> Height {
        match self {
            _ => unimplemented!(),
        }
    }

    pub fn set_height(&mut self, height: Height) {
        match self {
            Event::NewBlock(ev) => ev.set_height(height),
            _ => unimplemented!(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawObject {
    pub height: Height,
    pub action: String,
    pub idx: usize,
    pub events: HashMap<String, Vec<String>>,
}

impl RawObject {
    pub fn new(
        height: Height,
        action: String,
        idx: usize,
        events: HashMap<String, Vec<String>>,
    ) -> RawObject {
        RawObject {
            height,
            action,
            idx,
            events,
        }
    }
}

pub fn extract_events<S: ::std::hash::BuildHasher>(
    events: &HashMap<String, Vec<String>, S>,
    action_string: &str,
) -> Result<(), BoxError> {
    if let Some(message_action) = events.get("message.action") {
        if message_action.contains(&action_string.to_owned()) {
            return Ok(());
        }
        return Err("Missing action string".into());
    }
    Err("Incorrect Event Type".into())
}

#[macro_export]
macro_rules! make_event {
    ($a:ident, $b:literal) => {
        #[derive(Debug, Deserialize, Serialize, Clone)]
        pub struct $a {
            pub data: ::std::collections::HashMap<String, Vec<String>>,
        }
        impl ::std::convert::TryFrom<$crate::events::RawObject> for $a {
            type Error = ::anomaly::BoxError;

            fn try_from(result: $crate::events::RawObject) -> Result<Self, Self::Error> {
                match $crate::events::extract_events(&result.events, $b) {
                    Ok(()) => Ok($a {
                        data: result.events.clone(),
                    }),
                    Err(e) => Err(e),
                }
            }
        }
    };
}

#[macro_export]
macro_rules! attribute {
    ($a:ident, $b:literal) => {
        $a.events.get($b).ok_or($b)?[$a.idx].parse()?
    };
}

#[macro_export]
macro_rules! some_attribute {
    ($a:ident, $b:literal) => {
        $a.events
            .get($b)
            .map_or_else(|| None, |tags| tags[$a.idx].parse().ok())
    };
}
