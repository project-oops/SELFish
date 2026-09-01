# D087 - Native PS5 title manifests (`param.json`) and differential measurement over fake-signed packaging


**Native PS5 title manifests (`param.json`) and differential measurement over fake-signed packaging.**

A PS4 title runs inside the console's backward-compatibility container (`ps4_mode`), gated by legacy `param.sfo` metadata and Orbis sandbox caps. Building a native PS5 title requires current-generation title manifests (`param.json`), native title ID schemas (`PPSAxxxxx`/`NPXSxxxxx`), and registration through `sceAppInstUtilAppInstallTitleDir` into `/user/app/`.

Arbitrary unsigned code execution on retail PS5 kernels cannot directly run unsigned native ELF/SELF binaries due to ECDSA signature enforcement. Instead, the native PS5 title registration deep-links into an unsandboxed payload running in a `ps5_mode` context.

To make this verifiable without exfiltrating vendor binaries or keys:
1. `obSCEne`'s runtime implements an in-memory dynamic export resolver for `libkernel` anchored on `getpid` (`obs_bootstrap_payload_output`), resolving live `sceKernel*` calls to unblock probing in payload mode.
2. `047-reach` extends probes to GPU device nodes (`/dev/agc0` vs `/dev/gnm`), emitting an explicit `ps5_native` vs `ps4_compat` execution verdict.
3. `048-selfaudit` adds `metadata-differential` to classify installed titles on `/user/app` and `/system/vsh/app`, measuring whether titles declare `param.json` or `param.sfo` and identifying container generation markers (`54 14 F5 EE` vs `4F 15 3D 1D`).
4. `selfish-title` and `selfish-cli` provide native PS5 metadata construction and directory staging ready for `sceAppInstUtil` deployment.

Status: **done**.
