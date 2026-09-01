# D077 - The vendor segment begins with a fingerprint region, and leaving it out moves everything


Status: **decided**, 2026-08-29.

A real executable's string table does not start at offset zero of the vendor segment. It starts
at `0x18`, behind sixteen bytes of build identifier and eight of padding, and `DT_SCE_FINGERPRINT`
carries that region's offset - which is zero.

Omitting it shifts every table in the segment down by `0x18`, and the loader's layout
calculation refuses the file with `ENOEXEC` while printing that every tag it wanted was present.
The counts were all correct; the addresses they carried were not where the layout expected them.

The region is written as **zeroes**, like every digest and signature area here. A fingerprint
identifies a build and authenticates nothing, and a plausible-looking one would be a value
nothing here can justify. The console reads it back and reports it, which is how the reservation
was confirmed to be the right size:

```text
# /app0/eboot.bin
#  fingerprint: 0000000000000000000000000000000000000000
```

against a real library's `85b56188ae90acbce809687c007960fa00000000` - twenty bytes displayed,
sixteen of them meaningful, inside the `0x18` reserved.

With this, a package built entirely by this toolchain **installs, mounts, loads and executes**
on real hardware for the first time:

```text
<311> EXEC /app0/eboot.bin [user], vm#1, dmem#1 abi=ps4 category=ps4_game
[AppMgr Trace]: New Process, pid=0x137, created.  New App, appId=0x2018, created
[ResArbitrator] BigApp[0x2018] Changed state "RUNNING"
```

