# 7. Platform ABI declarations - deferred with a condition


Several hundred function signatures, declared in a probe to be *called* and implemented in an
emulator to be *provided*. They can disagree about an arity today, which is worse than the
magic bug: a wrong arity corrupts a stack and faults somewhere unrelated to the cause.

Deferred rather than forgotten, because the two consumers hold the same facts to deliberately
different standards. A shared set has to carry the **provenance level** with each signature.
(D003)

