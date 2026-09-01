# D086 - Confirm the format on the console and report the verdict - do not carry the bytes off


**Confirm the format on the console and report the verdict - do not carry the bytes off. The
dump (D085) was replaced by an on-console audit.**

D085 built a probe that copied a real SELF header off the console to a file, to be pulled to a
dev machine and read by `selfish audit`. It worked, and it was on the wrong side of the line: it
carried a vendor binary's bytes off the box to be read offline, which is the extraction the
provenance boundary exists to avoid - the drawn distinction being *measure what a call does*
(clean) versus *extract bytes and read them elsewhere* (grey). The reader who asked for it was
right, and it applied to the header dump as much as to the `.prx` mining that prompted it.

So `047-selfdump` is gone and `048-selfaudit` replaces it. It reads the header **transiently on
the console**, compares each fixed field against the value selfish's table pins, and reports
which the current generation keeps. The header bytes never land in a file and never leave the
box; only the finding does - which is the shape of every other check in the probe: call,
observe, report.

Three things make it clean rather than a dump wearing a hat:

- **The expected values flow to the console, not the other way.** selfish's table is public and
  cited; the vendor header is not. The comparison happens where the header already is, and only
  the yes/no comes back.
- **No second copy of the table.** `obscene-tool selfheader` projects
  `selfish_container::table::fixed_fields("self_header")` into `include/obscene/self_header.gen.h`
  at generate time, drift-gated by `verify.sh` the way every other generated header is. One
  source of truth (`data/self-format.tsv`), one comparison logic, reached from two languages.
- **A difference reports its measured value, and stops there.** "flags differs" without the
  value is not actionable, so the value is reported - as a format constant the probe observed,
  the way it reports every syscall return, not as a meaning read off the bytes. What a
  difference *means* at the new generation still needs a citable source (principle 1).

`048-reach` was added alongside it: a pure behavioural probe that opens paths straddling the
sandbox boundary and reports a plain **jailed / escaped** verdict, reading nothing. It sits
first so a `selfaudit` skip is explained - a skip is "the system eboot was not reachable from
here", which is itself the measurement.

Both compile for host and the freestanding console module under obSCEne's full `-Werror` set,
run in order, and skip cleanly where nothing is reachable. The compare logic was fired
positively against real SELFs through the generated header: a gen-5 and a gen-4 container both
confirm 9 of 9 fixed rows, and a corrupted `flags` byte diverges and reports the measured value.
Still **not run on hardware**: what a real console's SELF settles is the visit's to answer.

