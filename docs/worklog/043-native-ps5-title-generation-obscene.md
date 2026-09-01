# 2026-08-31 - Native PS5 Title Generation & obSCEne Self-Resolver Oracle


Implemented the payload in-memory self-resolver and differential probes in `obSCEne`, and expanded
native PS5 title metadata and generation in `selfish`:

1. **`obSCEne` Payload Self-Resolver (`runtime.c`)**:
   - `obs_bootstrap_payload_output` anchors `obs_libkernel_base_value` from `payload_args[0] - 0x5b0UL`.
   - `obs_resolve_payload_imports` scans the payload's in-memory ELF image and relocations (`.rela.dyn`/`.rela.plt`),
     resolving `libkernel` imports (`sceKernelOpen`, `sceKernelClose`, `sceKernelRead`, `sceKernelGetdents`, etc.)
     directly into the payload GOT, unblocking diagnostic probing under `elfldr`.
2. **`047-reach` & `048-selfaudit` Differential Oracles**:
   - `047-reach` probes `/dev/agc0` (PS5 RDNA2 GPU) and `/dev/gnm` (PS4 compat GPU), reporting a definitive
     `mode/verdict` (`ps5_native` vs `ps4_compat`).
   - `048-selfaudit` adds `metadata-differential` to classify installed titles across `/user/app` and
     `/system/vsh/app`, verifying `param.json` vs `param.sfo` distribution and container magics (`54 14 F5 EE` vs `4F 15 3D 1D`).
3. **`selfish-title` & `selfish-cli` Native PS5 Capabilities**:
   - Added native PS5 metadata helpers (`is_ps5_native`, `set_native_ps5`, `version`, `master_version`, `sdk_version`, `deeplink_uri`) in `selfish-title`.
   - Enhanced `selfish native` CLI command to stage complete native PS5 title directories ready for `sceAppInstUtilAppInstallTitleDir`.
   - All tests passing across all crates under `unsafe_code` forbidden and clippy clean under `-D warnings`. (D087)
