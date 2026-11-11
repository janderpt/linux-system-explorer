use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Browse(Option<PathBuf>),
    Help,
    Version,
}

pub const HELP: &str = "Linux System Explorer

Usage: linux-system-explorer [OPTIONS] [DIRECTORY]

  -h, --help       Show this help without opening a window
  -V, --version    Print the version without opening a window
  --               Treat the next argument as a directory, even if it starts with '-'

With no directory, open the user's home folder. Use trash: to open the Trash.
";

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Browse(None));
    };
    let command = if first == "--" {
        Command::Browse(Some(
            args.next().ok_or("Expected a directory after --")?.into(),
        ))
    } else if first == "--help" || first == "-h" {
        Command::Help
    } else if first == "--version" || first == "-V" {
        Command::Version
    } else if first.as_encoded_bytes().starts_with(b"-") {
        return Err("Unknown option; use --help or -- DIRECTORY");
    } else {
        Command::Browse(Some(first.into()))
    };
    if args.next().is_some() {
        return Err("Expected at most one directory");
    }
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_headless_commands_and_preserves_special_paths() {
        assert_eq!(parse(["--version".into()]), Ok(Command::Version));
        assert_eq!(parse(["--help".into()]), Ok(Command::Help));
        assert_eq!(parse([]), Ok(Command::Browse(None)));
        assert_eq!(
            parse(["--".into(), "-a folder".into()]),
            Ok(Command::Browse(Some("-a folder".into())))
        );
        assert!(parse(["--invalid".into()]).is_err());
        assert!(parse(["one".into(), "two".into()]).is_err());
        use std::os::unix::ffi::OsStringExt;
        let path = OsString::from_vec(b"/tmp/non-utf8-\xff".to_vec());
        assert_eq!(
            parse([path.clone()]),
            Ok(Command::Browse(Some(path.into())))
        );
    }
}
