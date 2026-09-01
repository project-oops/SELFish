# The executable format, and the container in both directions


`selfish-elf` (9 tests) and `selfish-container` (14) are in. 38 tests across the workspace,
clippy clean under `-D warnings`, `unsafe_code` forbidden.

The container reads as well as writes, and the round trip is a test rather than an aspiration:
build a container, parse it back, check the generation, the entry count, the stated size, and
that the inner executable is where the entry count says it should be. A writer checked only
against itself is checked against nothing.

**Byte-identical to the implementation it replaces**, on obSCEne's real 13MB module, for both
generations. That is the whole migration argument in one line - see D007.

### Surprise: two `pub` methods that nobody outside the crate could call

`Entry::carries_segment_data` and `Entry::segment_index` took a private `Constants`. Every
test passed, because tests live inside the module where that type is in scope. The public
surface exposed a type and withheld the means to interpret it, and only the compiler's
private-in-public check noticed.

The lesson is not about visibility rules. **An API that compiles is not an API somebody outside
can use**, and testing from inside the crate cannot tell the difference. (D006)

### Two bugs made structurally impossible rather than fixed

The `props: 0` failure - a container whose entries carry no flags, which parses perfectly and
then dies inside a loader's segment walk - now has a test that fails if no entry carries the
bit a loader searches for.

And `ObjectType` distinguishes executable from shared library in the type system, with a test
asserting they are not interchangeable. A loader that ignores the difference runs either
happily; only one that respects it reveals the mistake, as a program that loads, relocates,
runs its initialisers and does nothing at all.

