# D061 - The `param.sfo` field set was the previous generation's, and two homebrew packages said so


D059 built a `param.sfo` from `LibOrbisPkg`'s default field list: twelve fields. Both
current-generation packages to hand carry **twenty-nine**, and they agree with each other on
every one this crate does not have to guess.

Three of the differences would have shipped in silence:

| field | was written | both real samples |
|---|---|---|
| `SYSTEM_VER` | `0` | `0x8008000` |
| `APP_TYPE` | `4` | `1` |
| `ATTRIBUTE2` | *absent* | `0x400` |

plus `PUBTOOLVER`, `PUBTOOLMINVER`, `DEV_FLAG`, `REMOTE_PLAY_KEY_ASSIGN`, `PUBTOOLINFO`, seven
`SERVICE_ID_ADDCONT_ADD_*` slots and four `USER_DEFINED_PARAM_*` - all present in both samples
and none of them written here.

**A field that is absent is not a field that is empty.** The service-id slots hold empty strings
in every real file; leaving them out is a different table from leaving them blank, and only one
of those is what a console has seen before.

The values now live in a `measured` module with a note on each saying what the evidence was, and
a test pins them. Where the two samples disagreed - `ATTRIBUTE`, `DOWNLOAD_DATA_SIZE`,
`PUBTOOLINFO` - the field is title-specific and is left at a defensible minimum rather than
invented.

**This came from auditing homebrew that already ships**, which is worth recording as a method
rather than a one-off. A previous-generation default that parses cleanly and is wrong in three
fields is exactly the failure this repository was created to stop (the container magic, in the
`CLAUDE.md` opening). It was found by reading real files, not by reasoning about the format.

`PS4-Store`'s own `param.sfo` files were the other half of the check: they use `CATEGORY = gde`
where the current generation uses `gd`, which is what made the generation split visible in the
first place. Its `.gp4` also confirms the fake passcode independently - thirty-two ASCII zeros,
written out in the project file.

