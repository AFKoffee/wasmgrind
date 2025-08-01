# Native Thread Management
The native thread management implementation is straightforward and consists of a single map, which assigns thread-ids to `ConditionalHandles`.

When registering a new thread, i.e, when requesting a new thread-id for a child thread, a new record is inserted into this map consisting assigning a the new ID to an empty `ConditionalHandle`.

After the os-thread has been created, the returned `JoinHandle` is put into the `ConditionalHandle` associated with the newly created thread.

Any attempt to join the thread will now yield the conditional handle first, which can then be waited on until the os join handle is present. When the os join handle is present it can in turn be joined and after the function returns, there os-thread will have terminated successfully.

**Note:** Currently, the `ConditionalHandle` will _always_ contain a natve `JoinHandle` after `thread_create` returns. So if a the `JoinHandle` is not yet present when `thead_join` is called, this means that a thread tries to join a thread which it has not created. The current implementation does not prevent this, but it is of course madness! Anyhow, the use of `ConditionalHandles` for native thread management may thus be obsolete and the implementation details should be reconsidered.