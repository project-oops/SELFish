# D063 - One directory-entry writer, because the copy had the rule without the reasoning


`selfish-pfs` built directory entries in two places - `write` for the inner filesystem and
`outer` for the outer one. The logic was identical. The spelling was not: `write` used
`DIRENT_HEADER` and `DIRENT_ALIGN` with a comment explaining why an entry is padded, and
`outer` used bare `16` and `8` with nothing.

That is the dangerous shape of duplication. Two copies that *look* different are noticed; two
that differ only in whether the reasoning survived are not, and the unexplained one is the one
somebody later "simplifies". The `kind` and `imode` tables had drifted the same way.

`write` now owns all three and `outer` imports them.

The same pass removed two smaller things the audit turned up: `IMAGE_KEY_INDEX` existed twice
in `selfish-pkg` (public in `keys`, private in `write`, both `3`), and `playgo_len` still took
a `count` parameter it had stopped using when the image moved to a fixed offset - the argument
was being discarded with `let _ =`, which is a note saying "this is dead" written in a place
nobody reads.

