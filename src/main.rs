fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = lancev::Cli::parse(std::env::args().skip(1))?;
    if cli.help {
        print!("{}", lancev::Cli::HELP_TEXT);
        return Ok(());
    }
    ratatui::run(|terminal| lancev::run(terminal, cli.uri, cli.theme))
}
