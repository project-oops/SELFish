# Auditing the other constants found a second one that was never constant


D070's closing thought was that every value in `header_value` came from the same three large
packages, so each is a candidate for the same mistake. Checking them against the samples rather
than against each other found one immediately.

**`BODY_SIZE` was written as `0x7E000` and two of the three packages hold that.** The third holds
`0x57E000`. All three fit one rule:

```text
body_size = image_offset - body_offset

  item   524288 - 8192 =  516096   0x7E000
  lapy  5767168 - 8192 = 5758976   0x57E000
  store  524288 - 8192 =  516096   0x7E000
```

It is derived, and it had been right in every package this crate ever built purely because they
all put the image at `0x80000`. Nothing would have gone wrong until an image moved - and then the
failure would have been quiet, because `BODY_DIGEST` hashes exactly this region: a well-formed
digest covering the wrong bytes.

The digest now reads its length back out of the header instead of from the constant, so the two
cannot disagree. Writing a derived size and hashing a constant-sized region would be two answers
to one question, and the digest is the half that fails without saying so.

Both fixes verified against a rebuilt package: `body_size` comes out 516096 (unchanged, as it
must for an image at `0x80000`), and every digest still checks - 13/13 entry digests and both
manifest digests.

**The pattern worth keeping.** Two constants, both measured 2/3 or 3/3 from real files, both
actually derived. Agreement across a sample is evidence about the sample, and these three packages
vary in almost nothing: same image offset in two of them, same body offset in all three, all of
them tens of megabytes. The useful question about a constant here is not "how many packages agree"
but "what would have to differ for this to be wrong, and does any sample differ that way" - and
where the answer is *none of them do*, the value is unverified rather than confirmed.

