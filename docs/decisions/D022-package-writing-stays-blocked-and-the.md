# D022 - Package writing stays blocked, and the source that looked like it might unblock it does not


`ps5upload` turned up during the `PARAM.SFO` work - Rust, open-source, and it handles
packages. It is a **reader**. Its `build_pkg` functions are test fixtures that construct
enough of a header for its own parser, not a package anything would install.

So D012 stands unchanged: six of fourteen entries have no established meaning, two of those
are plausibly digest tables, and *plausibly* is not something to build on. What is needed is
still a packaging tool whose source says what goes in those entries.

Recorded because a near-miss that is not written down gets re-investigated.

