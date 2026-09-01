# D037 - A package writer that refuses to invent the entries nothing established explains


`write::Builder` lays out a package: header, entry table, and every entry this repository can
account for. The three with no established meaning - `0x400`, `0x401`, `0x1002` - must be
handed in, and a build without them fails **naming each one**, because the answer is always to
go and find them and a count does not say which.

Two design choices carry the principle rather than describing it:

- **A computed entry cannot also be supplied.** Passing `0x1` or `0x100` is an error, not an
  override. Two sources for one entry is exactly how a digest table stops matching the entries
  it describes, and the failure would be silent.
- **`Built::gaps` reports every region left blank.** Three slots of `0x80` are digests of
  something not in the package, so they are zero - and the builder says so, in the output, with
  the offset and length of each. A tool that quietly leaves holes hands the discovery to a
  console, which reports it as an install that did not work.

The strongest check is a test rather than an argument: `selfish derive` is run against a
package `write` produced, and every claim it re-derives from real packages holds. If the writer
and the derivation ever disagree, one of them is wrong and the suite says so.

What this does not do is sign, or produce the key entries. Those carry material wrapped under
the public fake keyset, and producing them is a separate problem from laying out a package.

