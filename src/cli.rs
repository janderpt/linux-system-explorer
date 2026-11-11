use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Browse(Option<PathBuf>),
    NewWindow(Option<PathBuf>),
    Reveal(Vec<PathBuf>, bool),
    FileManagerService,
    MountNetwork(String),
    ExportIcon(PathBuf),
    Help,
    Version,
}

pub const HELP: &str = "Linux System Explorer

Usage: linux-system-explorer [OPTIONS] [DIRECTORY]

  -h, --help       Show this help without opening a window
  -V, --version    Print the version without opening a window
  --select FILE... Open the parent folder with the given items selected
  --properties FILE Open its parent folder and show its properties
  --new-window [DIRECTORY] Always open a separate window, even if the folder is open
  --file-manager-service Run the headless FileManager1 D-Bus bridge
  --mount-network URI Internal GIO/GVfs mount helper with native authentication
  --export-icon PNG Write the canonical application icon without opening a window
  --               Treat the next argument as a directory, even if it starts with '-'

With no directory, open the user's home folder. Use trash: to open the Trash.
Opening a folder reuses a window already in that directory when available.
";

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, &'static str> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Browse(None));
    };
    let command = if first == "--new-window" {
        let next = args.next();
        let path = if next.as_deref() == Some(std::ffi::OsStr::new("--")) {
            Some(args.next().ok_or("Expected a directory after --")?.into())
        } else if next
            .as_ref()
            .is_some_and(|value| value.as_encoded_bytes().starts_with(b"-"))
        {
            return Err("Use --new-window -- DIRECTORY for a path beginning with '-'");
        } else {
            next.map(PathBuf::from)
        };
        Command::NewWindow(path)
    } else if first == "--select" || first == "--properties" {
        let paths: Vec<PathBuf> = args.map(PathBuf::from).collect();
        if paths.is_empty() || (first == "--properties" && paths.len() != 1) {
            return Err("Expected files after --select or exactly one file after --properties");
        }
        return Ok(Command::Reveal(paths, first == "--properties"));
    } else if first == "--file-manager-service" {
        Command::FileManagerService
    } else if first == "--mount-network" {
        Command::MountNetwork(
            args.next()
                .and_then(|arg| arg.into_string().ok())
                .ok_or("Expected a network URI after --mount-network")?,
        )
    } else if first == "--export-icon" {
        Command::ExportIcon(
            args.next()
                .ok_or("Expected a PNG path after --export-icon")?
                .into(),
        )
    } else if first == "--" {
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
        assert_eq!(parse(["--new-window".into()]), Ok(Command::NewWindow(None)));
        assert_eq!(
            parse(["--new-window".into(), "--".into(), "-folder".into()]),
            Ok(Command::NewWindow(Some("-folder".into())))
        );
        assert!(parse(["--new-window".into(), "--".into()]).is_err());
        assert!(parse(["--new-window".into(), "one".into(), "two".into()]).is_err());
        assert_eq!(
            parse(["--file-manager-service".into()]),
            Ok(Command::FileManagerService)
        );
        assert!(parse(["--file-manager-service".into(), "extra".into()]).is_err());
        assert_eq!(
            parse(["--mount-network".into(), "smb://server/share".into()]),
            Ok(Command::MountNetwork("smb://server/share".into()))
        );
        assert!(parse(["--mount-network".into()]).is_err());
        assert!(parse(["--mount-network".into(), "one".into(), "two".into()]).is_err());
        assert!(parse(["--select".into()]).is_err());
        assert!(parse(["--properties".into(), "a".into(), "b".into()]).is_err());
        assert_eq!(
            parse(["--select".into(), "-name".into(), "a space".into()]),
            Ok(Command::Reveal(
                vec!["-name".into(), "a space".into()],
                false
            ))
        );
        assert_eq!(
            parse(["--properties".into(), "/tmp/a".into()]),
            Ok(Command::Reveal(vec!["/tmp/a".into()], true))
        );
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
