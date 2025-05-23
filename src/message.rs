use bincode::{self, Decode, Encode};
use strum::Display;

#[derive(Debug, PartialEq, Eq, Display, Decode, Encode)]
pub enum Msg {
    PtyIn(Vec<u8>),
    PtyOut(Vec<u8>),
}