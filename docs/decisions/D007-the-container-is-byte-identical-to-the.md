# D007 - The container is byte-identical to the implementation it replaces


`selfish-container` and obSCEne's `mkself` were run over the same 13MB module for both
generations and compared:

```text
generation 4: IDENTICAL (10540336 bytes, magic 4f153d1d)
generation 5: IDENTICAL (10540336 bytes, magic 5414f5ee)
```

This is the migration evidence. A rewrite that is *probably* equivalent leaves a consumer
choosing between an untested new thing and a working old one, and the old one wins every
time. A rewrite that is provably identical on real input makes the switch uninteresting,
which is what a migration should be.

It also validates the reader against something other than its own writer. `mkself` was checked
by "a loader accepted it"; this is checked by "a byte-for-byte match with an independently
written implementation, plus a round trip through a parser that had never seen its output".

Worth keeping as a gate rather than a one-off. It cannot live in this repository - the module
is obSCEne's build output and is not committed anywhere - which makes it precisely the kind of
cross-repository test D005 says OOPS exists for.

Status: **decided**.

