// Implementation of a line sweep algorithm to find overlapping memory
// accesses in the trace.
// 
// The approach is based on the algorithms proposed by:
//  M. I. Shamos and D. Hoey, "Geometric intersection problems," 
//  17th Annual Symposium on Foundations of Computer Science (sfcs 1976), Houston, TX, USA, 1976, 
//  pp. 208-215, doi: 10.1109/SFCS.1976.16.
//
// Approach:
// 1.   Build a list of tuples (addr, start|end, i)
// 2.   Iterate over that list and build a set of active intervals
// 3.   If two intervals are in the active intervals set at the same time there is an overlap

use std::collections::HashSet;

use crate::tracing::metadata::MemoryRecord;

#[derive(PartialEq, Eq, Hash)]
enum IntervalEventType {
    Start,
    End,
}

impl Ord for IntervalEventType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (IntervalEventType::Start, IntervalEventType::Start) => std::cmp::Ordering::Equal,
            (IntervalEventType::Start, IntervalEventType::End) => std::cmp::Ordering::Greater,
            (IntervalEventType::End, IntervalEventType::Start) => std::cmp::Ordering::Less,
            (IntervalEventType::End, IntervalEventType::End) => std::cmp::Ordering::Equal,
        }
    }
}

impl PartialOrd for IntervalEventType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(PartialEq, Eq, Hash)]
struct IntervalEvent<'a> {
    addr: u32,
    ty: IntervalEventType,
    interval: &'a MemoryRecord,
}

impl Ord for IntervalEvent<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.addr.cmp(&other.addr) {
            std::cmp::Ordering::Equal => self.ty.cmp(&other.ty),
            ord => ord,
        }
    }
}

impl PartialOrd for IntervalEvent<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub fn line_sweep_algorithm<'a, I: IntoIterator<Item = &'a MemoryRecord>>(metadata: I) -> HashSet<(&'a MemoryRecord, &'a MemoryRecord)> {
    let mut events = Vec::new();

    for data in metadata {
        events.push(IntervalEvent {
            addr: data.wasm_id.address,
            ty: IntervalEventType::Start,
            interval: data,
        });

        events.push(IntervalEvent {
            addr: data.wasm_id.address + data.wasm_id.access_width,
            ty: IntervalEventType::End,
            interval: data,
        });
    }

    events.sort();

    let mut active = HashSet::new();
    let mut result =  HashSet::new();
    for event in events {
        match event.ty {
            IntervalEventType::Start => {
                assert!(active.insert(event.interval), "Interval should not be present here!");

                // We filter here for active intervals that are NOT equal to the current interval. WHY?
                // ==>  The reason for this analysis is to find memory accesses, which target the same memory but
                //      are not identified as such by the RapidBin format. This can happen because we assign a
                //      unique variable ID to every unique tuple (address, access width) -> our intervals.
                // ==>  We dont need to include EVERY interval that is shared amongst different threads because
                //      the concurrency analysis algorithm is aware of them through their unique variable ID.
                //      We only need to identify PAIRS OF DISTINCT intervals shared amongst different threads that
                //      still target the same memory regions (or at least partly the same memory regions) because
                //      those intervals (i.e. memory accesses) are treated as different variables by the RapidBin format
                //      and, therefore, by the algorithm.
                for interval in active.iter().filter(|interval| ***interval != *event.interval) {
                    result.insert((*interval, event.interval));
                }
            },
            IntervalEventType::End => {
                assert!(active.remove(&event.interval), "Interval should always be present here!");
            }
        }
    }

    assert!(
        active.is_empty(),
        "All intervals should be processed by now"
    );

    result
}
