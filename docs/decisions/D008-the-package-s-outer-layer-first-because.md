# D008 - The package's outer layer first, because it needs no cryptography


A package is four nested formats, and the three below the first need RSA, SHA-256, AES-CBC,
AES-XTS and zlib before a single filename can be listed. The outer container - header and
entry table - needs none of them.

Doing it alone buys three things. The reader can be **checked against real packages
immediately**, which it has been: all three current-generation samples parse, all fourteen
expected entries present in each, and one sample's nine extra identifiers correctly reported
as beyond the set. It keeps `selfish-pkg` free of cryptographic dependencies until something
actually requires them. And it establishes the entry table, which everything below has to
index into anyway.

### Two things the samples settled that no source stated

**The entry identifiers a package always carries.** Fourteen appeared in all three; nine more
in one only. That is a minimum viable package established by measurement, and it is what a
builder has to emit. No source consulted lists them - an extractor only needs the two that
unlock the filesystem, so it names those and ignores the rest.

**The image offset is a convention, not a field.** `0x700000` was searched for as both a
32-bit and a 64-bit value across the header and entry table of all three samples and found in
none, while all three carry high-entropy data at exactly that offset. Recorded as a fixed
convention with the evidence beside it rather than hardcoded silently, because the difference
between "this is fixed" and "we could not find where it is read from" matters to whoever
meets a package where it does not hold.

### Big-endian, unlike the executable container

Worth a decision entry rather than a comment, because it is the kind of thing that produces a
plausible wrong answer: read little-endian, an entry count of 14 becomes 234,881,024, and the
failure is an allocation rather than a parse error. The parser bounds the count against the
bytes actually present before allocating anything.

Status: **decided**.

