use std::path::Path;

pub fn ts_to_js(path: &Path, code: String) -> Result<String, anyhow::Error> {
    let parsed = deno_ast::parse_module(deno_ast::ParseParams {
        specifier: path.display().to_string(),
        text_info: deno_ast::SourceTextInfo::from_string(code),
        media_type: deno_ast::MediaType::TypeScript,
        capture_tokens: false,
        scope_analysis: false,
        maybe_syntax: None,
    })?;
    Ok(parsed.transpile(&Default::default())?.text)
}
