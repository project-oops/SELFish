# D035 - A derivation ships with the command that re-runs it


`selfish derive <package>...` re-checks every derived row against packages the reader supplies,
reports how many samples each claim survived, and exits non-zero if any fails. The rows in
`data/pkg-format.tsv` say `DERIVED` and name it.

The reason is the one this repository was founded on. A cited row can be checked by opening the
source it names. A **derived** row cannot - it is only as good as the samples behind it and the
method that produced it, and a derivation nobody can re-run is indistinguishable from somebody's
memory. Three packages backed these two; a fourth could kill them, and the command is how
anyone finds that out without taking a word for it.

It also refuses to be flattered. `Finding::survived` requires *every* testable sample to agree
rather than a majority, because a format true of two packages in three is not a format.

The precedent is obSCEne's `derive`, which re-derives the tag assignment from a module it just
wrote rather than trusting the constants that built it - and which caught this migration's
output as consistent when it could just as easily have caught it as wrong.

