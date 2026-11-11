use crate::{filesystem::Entry, icons::Icon};

/// Classify names without opening files or changing the system's application associations.
pub fn icon(entry: &Entry) -> Icon {
    if entry.is_dir {
        return Icon::Folder;
    }
    if matches!(
        entry.mode & 0o170000,
        0o010000 | 0o020000 | 0o060000 | 0o140000
    ) {
        return Icon::FileBinary;
    }
    let name = entry
        .display_path()
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "makefile"
            | "gnumakefile"
            | "cmakelists.txt"
            | "dockerfile"
            | "containerfile"
            | "justfile"
            | "jenkinsfile"
            | ".gitignore"
            | ".gitattributes"
            | ".editorconfig"
            | ".npmrc"
            | ".bashrc"
            | ".zshrc"
            | ".profile"
            | ".env"
    ) || name.starts_with(".env.")
    {
        return Icon::FileCode;
    }
    if matches!(
        name.as_str(),
        "readme" | "license" | "licence" | "copying" | "changelog" | "authors" | "notice" | "todo"
    ) {
        return Icon::FileText;
    }
    if matches!(
        name.as_str(),
        "id_rsa" | "id_ed25519" | "id_ecdsa" | "id_dsa"
    ) {
        return Icon::FileCertificate;
    }
    let executable = entry.mode & 0o111 != 0;
    match entry.extension().to_ascii_lowercase().as_str() {
        "pdf" => Icon::FilePdf,
        "doc" | "docx" | "docm" | "dot" | "dotx" | "odt" | "ott" | "rtf" | "pages" | "abw" => {
            Icon::FileDocument
        }
        "xls" | "xlsx" | "xlsm" | "xlsb" | "xlt" | "xltx" | "ods" | "ots" | "csv" | "tsv"
        | "numbers" => Icon::FileSpreadsheet,
        "ppt" | "pptx" | "pptm" | "pps" | "ppsx" | "pot" | "potx" | "odp" | "otp" => {
            Icon::FilePresentation
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz" | "lzma" | "tgz"
        | "tbz2" | "txz" | "cab" | "arj" => Icon::FileArchive,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "svgz" | "tif" | "tiff"
        | "ico" | "avif" | "heic" | "heif" | "jxl" | "raw" | "cr2" | "nef" | "dng" | "psd"
        | "xcf" | "eps" => Icon::FileImage,
        "mp3" | "wav" | "flac" | "ogg" | "oga" | "opus" | "m4a" | "aac" | "wma" | "aiff"
        | "aif" | "mid" | "midi" => Icon::FileAudio,
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "mpeg" | "mpg" | "m4v" | "wmv" | "flv" | "ogv"
        | "3gp" => Icon::FileVideo,
        "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" | "cs" | "java" | "kt" | "kts"
        | "scala" | "py" | "pyw" | "js" | "jsx" | "mjs" | "ts" | "tsx" | "go" | "rb" | "php"
        | "pl" | "lua" | "swift" | "dart" | "r" | "sh" | "bash" | "zsh" | "fish" | "ps1"
        | "bat" | "cmd" | "html" | "htm" | "css" | "scss" | "sass" | "less" | "xml" | "xsl"
        | "json" | "jsonc" | "yaml" | "yml" | "toml" | "ini" | "conf" | "config" | "cfg"
        | "properties" | "lock" | "env" | "vue" | "svelte" | "tex" | "ipynb" => Icon::FileCode,
        "txt" | "text" | "md" | "markdown" | "rst" | "log" | "nfo" | "asc" => Icon::FileText,
        "deb" | "rpm" | "pkg" | "msi" | "apk" | "flatpak" | "flatpakref" | "flatpakrepo"
        | "snap" | "jar" | "war" | "ear" => Icon::FilePackage,
        "exe" | "com" | "appimage" | "run" | "desktop" => Icon::FileExecutable,
        "pfx" | "p12" | "pem" | "crt" | "cer" | "der" | "pub" | "key" | "jks" | "keystore"
        | "p7b" | "p7c" => Icon::FileCertificate,
        "db" | "sqlite" | "sqlite3" | "mdb" | "accdb" | "sql" | "dump" => Icon::FileDatabase,
        "iso" | "img" | "dmg" | "qcow" | "qcow2" | "vmdk" | "vdi" | "vhd" | "vhdx" => {
            Icon::FileDisk
        }
        "drawio" | "dia" | "vsd" | "vsdx" | "odg" | "dwg" | "dxf" | "uml" | "mmd" | "mermaid" => {
            Icon::FileDiagram
        }
        "ttf" | "otf" | "woff" | "woff2" | "pfa" | "pfb" | "fon" => Icon::FileFont,
        "bin" if executable => Icon::FileExecutable,
        "bin" | "dat" | "dll" | "so" | "dylib" | "a" | "o" | "class" | "pyc" => Icon::FileBinary,
        "bak" | "backup" | "old" | "orig" => Icon::FileBackup,
        "ics" | "ical" | "vcf" | "vcard" => Icon::FileCalendar,
        _ if executable => Icon::FileExecutable,
        _ => Icon::File,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{
            ffi::OsStringExt,
            fs::{PermissionsExt, symlink},
        },
    };

    #[test]
    fn classifies_common_formats_case_insensitively_without_reading_content() {
        let root = tempfile::tempdir().unwrap();
        for (name, expected) in [
            ("report.PDF", Icon::FilePdf),
            ("report.docx", Icon::FileDocument),
            ("report.odt", Icon::FileDocument),
            ("budget.XLSX", Icon::FileSpreadsheet),
            ("data.csv", Icon::FileSpreadsheet),
            ("slides.odp", Icon::FilePresentation),
            ("archive.tar.gz", Icon::FileArchive),
            ("photo.JPG", Icon::FileImage),
            ("song.flac", Icon::FileAudio),
            ("movie.mkv", Icon::FileVideo),
            ("main.rs", Icon::FileCode),
            ("file.xml", Icon::FileCode),
            ("Dockerfile", Icon::FileCode),
            (".env.local", Icon::FileCode),
            ("CMakeLists.txt", Icon::FileCode),
            ("readme.md", Icon::FileText),
            ("app.deb", Icon::FilePackage),
            ("app.AppImage", Icon::FileExecutable),
            ("certificate.pfx", Icon::FileCertificate),
            ("data.sqlite", Icon::FileDatabase),
            ("linux.iso", Icon::FileDisk),
            ("diagram.drawio", Icon::FileDiagram),
            ("font.woff2", Icon::FileFont),
            ("data.Bin", Icon::FileBinary),
            ("archive.bak", Icon::FileBackup),
            ("calendar.ics", Icon::FileCalendar),
            ("unknown.unrecognized", Icon::File),
        ] {
            let path = root.path().join(name);
            fs::write(&path, "not inspected").unwrap();
            let entry = crate::filesystem::read_entry(&path).unwrap();
            assert_eq!(icon(&entry), expected, "{name}");
        }
    }

    #[test]
    fn directory_links_executables_and_non_utf8_names_keep_their_file_semantics() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("folder.pdf");
        fs::create_dir(&directory).unwrap();
        assert_eq!(
            icon(&crate::filesystem::read_entry(&directory).unwrap()),
            Icon::Folder
        );
        let linked = root.path().join("linked");
        symlink(&directory, &linked).unwrap();
        let entry = crate::filesystem::read_entry(&linked).unwrap();
        assert!(entry.is_symlink);
        assert_eq!(icon(&entry), Icon::Folder);
        let executable = root.path().join("program");
        fs::write(&executable, "program").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            icon(&crate::filesystem::read_entry(&executable).unwrap()),
            Icon::FileExecutable
        );
        let raw = root.path().join(std::ffi::OsString::from_vec(vec![
            0xff, b'.', b'p', b'd', b'f',
        ]));
        fs::write(&raw, "pdf").unwrap();
        assert_eq!(
            icon(&crate::filesystem::read_entry(&raw).unwrap()),
            Icon::FilePdf
        );
    }
}
