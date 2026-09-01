# D006 - `Entry` answers for itself, because the first version of it could not be called


`Entry::carries_segment_data` and `Entry::segment_index` originally took a `&Constants` - a
private type. Both methods were `pub` and neither was reachable: the compiler said so, and it
was right about something worse than visibility.

Those two methods answer the only question a consumer actually has about an entry. *Which
segment is this, and is it the one a loader will map?* Requiring a private argument to ask it
meant the crate exposed a type and withheld the means to interpret it.

They now read their own shifts from the format table. Marginally more work per call, and the
right shape: a consumer asks the entry, not the entry plus a handle to something it cannot
construct.

The general form, worth keeping: **an API that compiles is not the same as an API somebody
outside the crate can use.** These were written and tested from inside, where `Constants` is
in scope, so every test passed while the public surface was unusable.

Status: **decided**.

