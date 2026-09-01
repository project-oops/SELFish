# Two package magics, and a source that was not the one we needed


`ps5upload` - the second `PARAM.SFO` source - also parses packages, which looked for a moment
like the citable *writer* that has package writing blocked. It is not: it reads headers, and
its package builders are test fixtures. D012 stands. (D022)

It did supply one fact worth having. There is a **second package magic**, `\x7FFIH`, distinct
from `\x7FCNT` and current-generation. `selfish-pkg` would have called such a file "not a
package" - false about something the console installs. It now names it and refuses, with the
layout marked *not established* in the table rather than assumed. (D021)

The assumed-layout version of that mistake is the dangerous one: the header is big-endian, so
reading a count and a table offset out of the wrong header still produces a count, a table
offset, and entries. All wrong, none of it detectable.

