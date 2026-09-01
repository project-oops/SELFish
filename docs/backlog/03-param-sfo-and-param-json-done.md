# 3. ~~`param.sfo` and `param.json`~~ - done


Both are in `selfish-title`, read and written. Eleven real files round-trip byte for byte
across three hardware generations, and doing that overturned a rule both cited sources agree on
(D019) and found a value variant the first design collapsed (D020).

Left open deliberately **in the format crate**: which keys a title must carry. That is
convention rather than structure, it varies by generation and category, and a format crate that
asserted a required list would report the first title that omits one as malformed.

A *consumer* may answer it for its own case, and one has: `selfish_pkg::sfo` holds the field set
a package carries, measured from real current-generation packages. That is the split
`selfish-title`'s header asks for - and a session that missed it wrote a second `PARAM.SFO`
implementation instead, which is now deleted. (D061, D062)

