fn main() {
    std::process::exit(recdr::cli::run(no_mcp))
}

fn no_mcp() -> recdr::Result<()> {
    Err(recdr::Error::Usage(
        "the mcp command is not built yet".into(),
    ))
}
