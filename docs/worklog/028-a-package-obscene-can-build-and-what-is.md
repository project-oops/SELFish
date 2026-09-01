# A package obSCEne can build, and what is still placeholder


`make pkg` in obSCEne now produces `obscene.pkg` end to end: the eboot and a `param.json` become
an app tree, the tree becomes a filesystem image, and the image becomes a package. Nothing in
that chain is stubbed any more.

Three defects were closed on the way, and all three were **certain** rather than probable
failures on hardware. That distinction matters: none of them would have shown up in any test
here, because the reader this crate ships takes shortcuts a console cannot.

- **The key blobs were zeros.** `0x10` and `0x20` carry what a console unwraps to reach the
  filesystem. Now computed, and reproduced **byte-for-byte** against two real packages - 2048
  of 2048 and 256 of 256. (D054)
- **The header past `0x410` was zero**, including `pfs_image_size`. There was nothing to mount.
  Now measured out of a real package and matching offset for offset. (D056)
- **The flat path table was a placeholder.** Built properly now, *because* nothing here reads
  it - a wrong one fails only on the console. (D057)

### Surprises

**The blocker that looked hardest was a public-key operation.** Writing the key blobs needs only
public halves, and a public key cannot unwrap. The thing that had been described as "material
wrapped under keys we take as input" was a wrap this repository could always have performed.

**Deterministic padding turned a weak check into a strong one.** The RSA filler comes from a
seeded Mersenne Twister, so there is exactly one correct answer and it can be compared against
real bytes. Had the padding been random, the best available check would have been "it decrypts
back", which a self-consistently wrong implementation also passes.

**Two tests earned their keep in one session.** The derive-against-own-output test caught the
block-digest table going stale when the image moved to `0x80000`. A new test that keys a package
with a non-fake passcode caught the entry encryption still hardcoding the fake one - every other
test uses the fake passcode and none of them could have found it. (D055)

**A concurrent session had already written the obSCEne half.** `scripts/build-pkg.sh` existed,
correct, waiting on a command it named in a comment. It was completed rather than replaced, and
its one latent bug - building the image without a content id - was fixed with a note saying why
that produces two files which each look fine. (D058)

### What is still placeholder, and should be said out loud

- `param.sfo`, `icon0.png` and the playgo pair are stand-ins. A console reads `param.sfo` for
  the title id and version, so it is the **most likely next rejection** now the container is
  right. These are the title's content, not the format's, and the library refuses to invent
  them.
- Three `0x80` slots still digest something not present in any package. `is_complete()` returns
  `false` and the builder names them.
- Payloads past ~117 MiB need a doubly-indirect signature block, which returns an error.
- **Nothing built here has ever been installed on hardware.**

