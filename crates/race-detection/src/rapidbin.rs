/// Utilities to parse execution traces in RapidBin format.
pub mod parser;

/// Utilities to encode execution traces to RapidBin format.
pub mod encoder;

// ============================================================================
// Statics, which are relevant for reading and writing traces in RapidBin format:
const THREAD_NUM_BITS: i16 = 10;
const THREAD_BIT_OFFSET: i16 = 0;

const OP_NUM_BITS: i16 = 4;
const OP_BIT_OFFSET: i16 = THREAD_BIT_OFFSET + THREAD_NUM_BITS;

const DECOR_NUM_BITS: i16 = 34;
const DECOR_BIT_OFFSET: i16 = OP_BIT_OFFSET + OP_NUM_BITS;

const LOC_NUM_BITS: i16 = 15;
const LOC_BIT_OFFSET: i16 = DECOR_BIT_OFFSET + DECOR_NUM_BITS;
// ============================================================================
// Only relevant for reading traces:
const NUMBER_OF_TRHEADS_MASK: i16 = 0x7FFF;
const NUMBER_OF_LOCKS_MASK: i32 = 0x7FFFFFFF;
const NUMBER_OF_VARS_MASK: i32 = 0x7FFFFFFF;
const NUMBER_OF_EVENTS_MASK: i64 = 0x7FFFFFFFFFFFFFFF;

const THREAD_MASK: i64 = ((1 << THREAD_NUM_BITS) - 1) << THREAD_BIT_OFFSET;
const OP_MASK: i64 = ((1 << OP_NUM_BITS) - 1) << OP_BIT_OFFSET;
const DECOR_MASK: i64 = ((1 << DECOR_NUM_BITS) - 1) << DECOR_BIT_OFFSET;
const LOC_MASK: i64 = ((1 << LOC_NUM_BITS) - 1) << LOC_BIT_OFFSET;
// ============================================================================
