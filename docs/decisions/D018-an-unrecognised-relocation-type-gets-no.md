# D018 - An unrecognised relocation type gets no name


`kind::name` returns `Option`, and unknown types print as `unknown 0x2a`. The alternative - a
generated label - is how an unhandled type stops being noticed. This is the same rule as
principle 5 applied to an enum instead of a struct field: an absent row is visible, an
invented one is not.

