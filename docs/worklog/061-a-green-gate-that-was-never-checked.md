# 2026-09-10 - A green gate that was never checked, and a commit that took somebody else's work


Every commit today reported "gate passed". One of them did not, and the reason it still committed
is a shell mistake that had been in every command since the morning:

```bash
./bin/selfish check 2>&1 | tail -4 && git commit ...
```

**A pipeline's exit status is the last command's.** `tail` always succeeds, so `check` returning
1 was invisible and the `&&` never guarded anything. It went unnoticed for twenty-three commits
because the gate genuinely was green every time - the mistake only became visible once it was
not.

The second half is worse. The commit was meant to touch one markdown file and contained seven:

```
crates/selfish-pkg/examples/extract_eboot.rs  +66   (untracked all session, deliberately excluded)
crates/selfish-title/src/lib.rs, param.rs     +64
docs/GLOSSARY.md, docs/features/writing.md    +33
selfish-cli/src/main.rs                         9
```

None of it was this session's work. Another agent was editing the same tree - a glossary entry on
`applicationCategoryType` and DMEM budgets, changes to `param.rs` and the CLI - and `git add -A`
swept all of it under an unrelated message. The `:!` exclusion that had kept `extract_eboot.rs`
untracked for the whole session did not hold either.

So the red gate was *their* files caught mid-edit, and the commit that captured them is the same
commit that failed to notice.

## What was done about it

Nothing to the history. Reverting or resetting would rewrite work belonging to somebody who may
still have been holding it, which is a larger risk than an untidy commit, and it is not this
session's to take. The operator's instruction was to leave the commit and fix the gate.

The gate fix is three mechanical rustfmt hunks - two import orderings and a blank line - in the
two files that were caught unformatted. `cargo fmt`, exit 0; `./bin/selfish check`, **exit code
read directly rather than through a pipe**, exit 0, 257 tests.

## What actually failed

Not the tooling. The gate worked correctly every time it ran; it reported 1 and was asked in a
way that could not hear it.

**A check whose result is discarded is not a check, and it looks exactly like one that passes.**
The output even said `gate passed` on the runs that did - which is precisely why nothing seemed
wrong: the happy path printed the same thing as the broken path for twenty-three commits.

That is the day's own theme arriving from the process side rather than the format side. `data/`
carried a claim nobody checked, a `Display` explained something nobody printed, a test comment
held a warning nobody read - and a gate returned a number nobody looked at. Each was recorded
correctly, in a place that was not consulted.

## And the part that is specific to working alongside others

`git add -A` describes the *tree*, not the author's intent, and this session had been treating
them as the same thing. That is safe exactly as long as nobody else is editing, which was true
until it silently was not. There was no signal - no conflict, no warning - just a commit that was
bigger than it should have been and a stat line nobody would read unless something else had
already gone wrong.

Staging by explicit path costs one line and cannot do this.
