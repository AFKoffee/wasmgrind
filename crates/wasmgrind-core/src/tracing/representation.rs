use bitcode::{Decode, Encode};

/// A enum of operations that can be part of an event.
#[derive(Encode, Decode, Debug, PartialEq, Eq, Clone, Hash)]
pub enum Op {
    /// A _read_ of `n` bytes occured beginning at address `addr`.
    Read { addr: u32, n: u32, atomic: bool },

    /// A _write_ of `n` bytes occured beginning at address `addr`.
    Write { addr: u32, n: u32, atomic: bool },

    /// The mutex with id `lock` was aquired
    Aquire { lock: u32 },

    /// The mutex with id `lock` was requested to be aquired
    Request { lock: u32 },

    /// The mutex with id `lock` was released
    Release { lock: u32 },

    /// A thread with id `tid` was spawned
    Fork { tid: u32 },

    /// A thread with id `tid` was joined
    Join { tid: u32 },
}

/// A single event of the execution trace.
#[derive(Encode, Decode, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Event {
    pub t: u32,          // ID of the executing thread
    pub op: Op,          // executed operation
    pub loc: (u32, u32), // location in the program: (function_idx, instr_idx)
}
