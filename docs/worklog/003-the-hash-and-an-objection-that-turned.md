# The hash, and an objection that turned out to argue the other way


`selfish-nid` reproduces all 389 harvested name-and-encoding pairs, in both directions.

**Surprise: the duplication that was being removed had never caught anything.** The probe's
header recorded that two implementations were deliberate, so a measurement tool would not
share a hash with the thing it measures - a genuinely good reason, not a stale one. But the
same note records that the byte order was once documented one way and implemented the other,
and went unnoticed *because neither implementation had a fixture*. Two implementations did not
find it. A fixture would have, immediately.

So the fixture is not a consolation for losing the duplication. It is the thing that was
missing all along, and it is strictly stronger: agreement between two of our own
implementations is evidence about us, while agreement with 389 pairs produced by other people's
code is evidence about the algorithm.

The test asserts the pair *count* as well as the pairs. A fixture file that silently stopped
being read would otherwise pass while proving nothing.

