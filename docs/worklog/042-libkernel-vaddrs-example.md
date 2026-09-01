# 2026-08-31 - `libkernel_vaddrs` example


A `selfish-container` example printing every defined export of a module as `encoded-NID vaddr`
(`section != 0`, `value != 0`), built on `dynamic::symbols`. The raw form that proves the vaddr
measurement; naming the NIDs needs the mined corpus and happens in obSCEne (`obscene-tool
vaddrs`). See D088. Clippy-clean under the gate.

