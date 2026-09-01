# D031 - The command-line tool does not panic when a pipe closes


`println!` panics on `EPIPE`, so `selfish imports big.prx | head` ended in a backtrace instead
of the four lines that were asked for. That is not cosmetic in a binary whose stated purpose is
being pointed at real material - paging its output is the normal way to look at it, and it was
hit repeatedly while checking this repository's own work.

The usual fix is restoring the default `SIGPIPE` disposition, which needs `unsafe` and a libc
dependency. This workspace forbids the first and does not want the second for one signal, so
output goes through a `say!` macro that writes to a locked stdout and exits **zero** when the
write fails.

Zero rather than an error: a reader that stopped reading got what it wanted, and a non-zero
status would make every `| head` look like a failed command inside a script.

