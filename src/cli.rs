use std::{error::Error, io, io::ErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupTheme {
    Light,
    Dark,
}

impl StartupTheme {
    pub const fn light_mode(self) -> bool {
        matches!(self, Self::Light)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cli {
    pub uri: Option<String>,
    pub theme: Option<StartupTheme>,
    pub help: bool,
}

impl Cli {
    pub const HELP_TEXT: &str = "Usage: lancev [OPTIONS]\n\nOptions:\n      --uri <URI>            Lance dataset URI\n  -t, --theme <light|dark>   Startup theme (default: dark)\n  -h, --help                 Print help\n";

    pub fn parse<I>(args: I) -> Result<Self, Box<dyn Error>>
    where
        I: IntoIterator<Item = String>,
    {
        let mut args = args.into_iter();
        let mut uri = None;
        let mut theme = None;
        let mut help = false;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--uri" => {
                    let value = next_value(&mut args, "--uri requires a URI value")?;
                    if uri.replace(value).is_some() {
                        return Err(invalid("--uri may only be provided once"));
                    }
                }
                "--theme" | "-t" => {
                    let value = next_value(&mut args, "--theme requires a theme value")?;
                    let parsed_theme = parse_theme(&value)?;
                    if theme.replace(parsed_theme).is_some() {
                        return Err(invalid("--theme may only be provided once"));
                    }
                }
                "--help" | "-h" => help = true,
                _ if arg.starts_with("--uri=") => {
                    let value = arg["--uri=".len()..].to_string();
                    if value.is_empty() {
                        return Err(invalid("--uri requires a non-empty URI value"));
                    }
                    if uri.replace(value).is_some() {
                        return Err(invalid("--uri may only be provided once"));
                    }
                }
                _ if arg.starts_with("--theme=") => {
                    let parsed_theme = parse_theme(&arg["--theme=".len()..])?;
                    if theme.replace(parsed_theme).is_some() {
                        return Err(invalid("--theme may only be provided once"));
                    }
                }
                _ if arg.starts_with("-t=") => {
                    let parsed_theme = parse_theme(&arg["-t=".len()..])?;
                    if theme.replace(parsed_theme).is_some() {
                        return Err(invalid("--theme may only be provided once"));
                    }
                }
                _ => return Err(invalid(format!("unknown argument: {arg}"))),
            }
        }

        Ok(Self { uri, theme, help })
    }
}

fn next_value<I>(args: &mut I, error_message: &str) -> Result<String, Box<dyn Error>>
where
    I: Iterator<Item = String>,
{
    args.next()
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .ok_or_else(|| invalid(error_message))
}

fn parse_theme(value: &str) -> Result<StartupTheme, Box<dyn Error>> {
    match value {
        "light" => Ok(StartupTheme::Light),
        "dark" => Ok(StartupTheme::Dark),
        _ => Err(invalid("--theme must be either light or dark")),
    }
}

fn invalid(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidInput, message.into()))
}

#[cfg(test)]
mod tests {
    use super::{Cli, StartupTheme};

    #[test]
    fn parses_uri_argument() {
        assert_eq!(
            Cli::parse(["--uri".to_string(), "file:///tmp/data".to_string()]).unwrap(),
            Cli {
                uri: Some("file:///tmp/data".to_string()),
                theme: None,
                help: false,
            }
        );
    }

    #[test]
    fn parses_equals_form() {
        assert_eq!(
            Cli::parse(["--uri=s3://bucket/table".to_string()]).unwrap(),
            Cli {
                uri: Some("s3://bucket/table".to_string()),
                theme: None,
                help: false,
            }
        );
    }

    #[test]
    fn parses_theme_options() {
        assert_eq!(
            Cli::parse(["--theme".to_string(), "dark".to_string()]).unwrap(),
            Cli {
                uri: None,
                theme: Some(StartupTheme::Dark),
                help: false,
            }
        );
        assert_eq!(
            Cli::parse(["-t".to_string(), "light".to_string()]).unwrap(),
            Cli {
                uri: None,
                theme: Some(StartupTheme::Light),
                help: false,
            }
        );
        assert_eq!(
            Cli::parse(["--theme=dark".to_string()]).unwrap(),
            Cli {
                uri: None,
                theme: Some(StartupTheme::Dark),
                help: false,
            }
        );
    }

    #[test]
    fn rejects_unknown_theme() {
        let error = Cli::parse(["--theme".to_string(), "blue".to_string()])
            .unwrap_err()
            .to_string();

        assert_eq!(error, "--theme must be either light or dark");
    }

    #[test]
    fn parses_help_options() {
        for option in ["--help", "-h"] {
            assert_eq!(
                Cli::parse([option.to_string()]).unwrap(),
                Cli {
                    uri: None,
                    theme: None,
                    help: true,
                }
            );
        }
    }
}
