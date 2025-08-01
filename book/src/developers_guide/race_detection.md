# Race Detection
This crate encapsulates the functionality to record execution traces that allow to perform offline analysis of program runs. This allows the user to check various race conditions after the program exited.

It is split into a library that can be used to build traces and emit them in RapidBin format and a simple command line tool, which is able to parse a binary execution trace and print it to the terminal.

The tracing needs to have as little runtime overhead as possible, which is why the trace has a specific internal data structure during the recording and gets transformed into the RapidBin format afterwards.