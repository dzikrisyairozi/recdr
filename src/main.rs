fn main() {
    recdr::platform::set_dpi_aware();
    std::process::exit(recdr::cli::run(recdr::mcp::serve, recdr::gui::run))
}
