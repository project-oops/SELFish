//! `shader`: an AGC shader container from a bytecode size and registers.
//!
//! The build-time counterpart of `selfish-shader` for a consumer that is not Rust: obSCEne's
//! probe generates its container here rather than carrying a copy of the layout. (D103)

use std::path::Path;

use crate::Result;

/// The `shader` subcommand's arguments.
pub(crate) struct Request<'a> {
    pub(crate) stage: &'a str,
    pub(crate) code: Option<&'a Path>,
    pub(crate) shader_size: Option<u32>,
    pub(crate) target: &'a str,
    pub(crate) sh_registers: &'a [String],
    pub(crate) out: &'a Path,
}

/// `selfish shader`.
pub(crate) fn build(request: &Request<'_>) -> Result {
    let (stage, label) = stage(request.stage)?;
    let size = match (request.code, request.shader_size) {
        (Some(path), _) => u32::try_from(std::fs::read(path)?.len())
            .map_err(|_| "the shader bytecode is larger than shader_size (a u32) can record")?,
        (None, Some(size)) => size,
        (None, None) => return Err("one of --code or --shader-size is required".into()),
    };
    let target = hex_or_dec_u32(request.target)
        .ok_or_else(|| format!("bad --target {:?}", request.target))?;
    let registers = registers(request.sh_registers)?;

    // The hardware refuses a shader whose SH register table lacks its stage's program-address
    // pair. Warned rather than refused, because this tool lays out what it is handed and does
    // not own that rule. Only compute's pair (0x20c/0x20d) is known here. (D103)
    if registers.len() < 2 {
        let hint = if stage == selfish_shader::STAGE_COMPUTE {
            " - for compute that is 0x20c=0, 0x20d=0"
        } else {
            ""
        };
        say!(
            "warning: {} SH register(s). A console needs at least the stage's program-address pair, \
             which it patches from the code pointer, plus the shader's own resource registers{hint} \
             - pass them with --sh-reg",
            registers.len()
        );
    }

    let container = selfish_shader::Container::for_stage(stage, size, target, registers).build();
    crate::pipeline::write_exactly(request.out, &container)?;
    say!(
        "{}: {} byte AGC {label} container (shader_size {size}, target {target:#x})",
        request.out.display(),
        container.len()
    );
    Ok(())
}

/// A named stage's citable type value, or a raw value the caller has confirmed itself.
fn stage(name: &str) -> Result<(u8, String)> {
    Ok(match name {
        "compute" => (selfish_shader::STAGE_COMPUTE, "compute".to_owned()),
        "pixel" => (selfish_shader::STAGE_PIXEL, "pixel".to_owned()),
        "vertex" => (selfish_shader::STAGE_VERTEX, "vertex".to_owned()),
        other => {
            let value = hex_or_dec_u32(other)
                .and_then(|v| u8::try_from(v).ok())
                .ok_or_else(|| format!("bad --stage {other:?}: a name or a 0-255 type value"))?;
            (value, format!("type {value:#x}"))
        }
    })
}

/// `OFFSET=VALUE` specs as registers.
fn registers(specs: &[String]) -> Result<Vec<selfish_shader::ShaderRegister>> {
    specs
        .iter()
        .map(|spec| {
            let (offset, value) = spec
                .split_once('=')
                .ok_or_else(|| format!("--sh-reg wants OFFSET=VALUE, got {spec:?}"))?;
            Ok(selfish_shader::ShaderRegister {
                offset: hex_or_dec_u32(offset)
                    .ok_or_else(|| format!("bad register offset {offset:?}"))?,
                value: hex_or_dec_u32(value)
                    .ok_or_else(|| format!("bad register value {value:?}"))?,
            })
        })
        .collect()
}

/// A `0x`-prefixed hexadecimal or plain decimal `u32`.
pub(crate) fn hex_or_dec_u32(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    trimmed.strip_prefix("0x").map_or_else(
        || trimmed.parse::<u32>().ok(),
        |hex| u32::from_str_radix(hex, 16).ok(),
    )
}
