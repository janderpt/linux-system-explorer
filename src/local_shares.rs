//! Manage Samba usershares as the desktop user, without passwords or elevation.
use std::{
    ffi::OsString,
    io,
    path::PathBuf,
    process::{Command, Stdio},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Deny,
}
impl Access {
    pub const ALL: [Self; 3] = [Self::Read, Self::Write, Self::Deny];
    pub fn label(self) -> &'static str {
        match self {
            Self::Read => crate::i18n::tr("Somente leitura"),
            Self::Write => crate::i18n::tr("Leitura e escrita"),
            Self::Deny => crate::i18n::tr("Sem acesso"),
        }
    }
    fn code(self) -> char {
        match self {
            Self::Read => 'R',
            Self::Write => 'F',
            Self::Deny => 'D',
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub principal: String,
    pub access: Access,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share {
    pub name: String,
    pub path: PathBuf,
    pub comment: String,
    pub rules: Vec<Rule>,
    pub guests: bool,
    pub editable: bool,
}
impl Share {
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let user = uzers::get_current_username()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Self {
            name,
            path,
            comment: String::new(),
            rules: vec![Rule {
                principal: user,
                access: Access::Write,
            }],
            guests: false,
            editable: true,
        }
    }
    pub fn address(&self, host: &str) -> String {
        let mut url = url::Url::parse(&format!("smb://{host}/")).expect("validated local host");
        url.set_path(&format!("/{}", self.name));
        url.to_string()
    }
    pub fn windows_address(&self, host: &str) -> String {
        format!("\\\\{host}\\{}", self.name)
    }
    pub fn validate(&self) -> io::Result<()> {
        validate_name(&self.name)?;
        if !self.path.is_absolute() || !self.path.is_dir() {
            return Err(invalid(
                "A pasta compartilhada não existe ou não é um caminho absoluto.",
            ));
        }
        if self.comment.chars().any(char::is_control) || self.comment.len() > 256 {
            return Err(invalid(
                "A descrição deve ter até 256 bytes e não pode conter quebras de linha.",
            ));
        }
        if self.rules.is_empty() {
            return Err(invalid(
                "Adicione pelo menos um usuário ou grupo às permissões.",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for rule in &self.rules {
            if rule.principal.trim().is_empty()
                || rule.principal != rule.principal.trim()
                || rule
                    .principal
                    .chars()
                    .any(|c| c.is_control() || matches!(c, ',' | ':' | ';'))
                || !seen.insert(rule.principal.to_lowercase())
            {
                return Err(invalid(
                    "Informe usuários ou grupos distintos, sem vírgulas, dois-pontos ou quebras de linha.",
                ));
            }
        }
        Ok(())
    }
    fn acl(&self) -> String {
        // Windows share ACLs evaluate in order: explicit denies must precede grants.
        self.rules
            .iter()
            .filter(|r| r.access == Access::Deny)
            .chain(self.rules.iter().filter(|r| r.access != Access::Deny))
            .map(|r| format!("{}:{}", r.principal, r.access.code()))
            .collect::<Vec<_>>()
            .join(",")
    }
}
fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, crate::i18n::tr(message))
}
fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 80
        || name != name.trim()
        || matches!(name, "." | "..")
        || name.starts_with('-')
        || name.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '/' | '\\'
                        | '['
                        | ']'
                        | ':'
                        | ';'
                        | '"'
                        | '<'
                        | '>'
                        | '|'
                        | '*'
                        | '?'
                        | '='
                        | '+'
                        | ','
                        | '\0'
                )
        })
    {
        return Err(invalid(
            "Use um nome de até 80 bytes, sem separadores de caminho ou caracteres reservados.",
        ));
    }
    Ok(())
}
fn parse(text: &str) -> io::Result<Vec<Share>> {
    let mut shares = Vec::new();
    let mut current: Option<Share> = None;
    let malformed = || invalid("O Samba retornou uma definição de compartilhamento inválida.");
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(share) = current.take() {
                shares.push(share);
            }
            let name = &line[1..line.len() - 1];
            validate_name(name)?;
            current = Some(Share {
                name: name.into(),
                path: PathBuf::new(),
                comment: String::new(),
                rules: vec![],
                guests: false,
                editable: false,
            });
        } else {
            let share = current.as_mut().ok_or_else(malformed)?;
            let (key, value) = line.split_once('=').ok_or_else(malformed)?;
            match key {
                "path" => share.path = PathBuf::from(value),
                "comment" => share.comment = value.into(),
                "guest_ok" => {
                    share.guests = match value {
                        "y" | "yes" => true,
                        "n" | "no" => false,
                        _ => return Err(malformed()),
                    }
                }
                "usershare_acl" => {
                    share.rules = value
                        .trim_end_matches(',')
                        .split(',')
                        .map(|entry| {
                            let (principal, code) = entry.rsplit_once(':').ok_or_else(malformed)?;
                            let access = match code {
                                "R" => Access::Read,
                                "F" => Access::Write,
                                "D" => Access::Deny,
                                _ => return Err(malformed()),
                            };
                            if principal.is_empty() {
                                return Err(malformed());
                            }
                            Ok(Rule {
                                principal: principal.into(),
                                access,
                            })
                        })
                        .collect::<io::Result<_>>()?;
                }
                _ => (),
            }
        }
    }
    if let Some(share) = current {
        shares.push(share);
    }
    let mut seen = std::collections::BTreeSet::new();
    if shares
        .iter()
        .any(|s| !s.path.is_absolute() || s.rules.is_empty() || !seen.insert(s.name.to_lowercase()))
    {
        return Err(malformed());
    }
    Ok(shares)
}

pub(crate) trait Runner {
    fn run(&self, args: Vec<OsString>) -> io::Result<String>;
}
#[derive(Default)]
pub struct SystemNet;
impl Runner for SystemNet {
    fn run(&self, args: Vec<OsString>) -> io::Result<String> {
        use std::io::Read;
        let stdout = tempfile::tempfile()?;
        let stderr = tempfile::tempfile()?;
        let mut child = Command::new("net").arg("usershare").args(args).env("LC_ALL", "C")
            .stdin(Stdio::null()).stdout(stdout.try_clone()?).stderr(stderr.try_clone()?).spawn().map_err(|e| {
                io::Error::new(e.kind(), format!("{} {e}", crate::i18n::tr("Não foi possível executar o Samba. Verifique a instalação de samba-common-bin.")))
            })?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    crate::i18n::tr(
                        "O Samba demorou para responder. Atualize a lista para conferir o resultado antes de tentar novamente.",
                    ),
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        use std::io::{Seek, SeekFrom};
        let mut stdout = stdout;
        let mut stderr = stderr;
        stdout.seek(SeekFrom::Start(0))?;
        stderr.seek(SeekFrom::Start(0))?;
        let mut out = Vec::new();
        let mut err = Vec::new();
        stdout.take(4 * 1024 * 1024).read_to_end(&mut out)?;
        stderr.take(4096).read_to_end(&mut err)?;
        if !status.success() {
            return Err(io::Error::other(format!(
                "{} {}",
                crate::i18n::tr(
                    "O Samba recusou a operação. Verifique o serviço, o grupo sambashare e as permissões da pasta."
                ),
                String::from_utf8_lossy(&err).trim()
            )));
        }
        String::from_utf8(out)
            .map_err(|_| invalid("O Samba retornou texto que não pode ser exibido em UTF-8."))
    }
}
pub struct Service<R = SystemNet> {
    runner: R,
}
impl Default for Service {
    fn default() -> Self {
        Self { runner: SystemNet }
    }
}
impl<R: Runner> Service<R> {
    pub fn list(&self) -> io::Result<Vec<Share>> {
        let mut all = parse(&self.runner.run(vec!["info".into(), "--long".into()])?)?;
        let own = parse(&self.runner.run(vec!["info".into()])?)?;
        for share in &mut all {
            share.editable = own
                .iter()
                .any(|s| s.name == share.name && s.path == share.path);
        }
        all.sort_by_key(|s| s.name.to_lowercase());
        Ok(all)
    }
    fn ensure_current(&self, expected: &Share, shares: &[Share]) -> io::Result<()> {
        if !expected.editable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                crate::i18n::tr("Somente o proprietário pode alterar este compartilhamento."),
            ));
        }
        if !shares.iter().any(|s| s == expected && s.editable) {
            return Err(io::Error::other(crate::i18n::tr(
                "O compartilhamento foi alterado por outro aplicativo. Atualize a lista antes de continuar.",
            )));
        }
        Ok(())
    }
    fn add(&self, share: &Share) -> io::Result<()> {
        self.runner.run(vec![
            "add".into(),
            share.name.clone().into(),
            share.path.as_os_str().into(),
            share.comment.clone().into(),
            share.acl().into(),
            if share.guests {
                "guest_ok=y"
            } else {
                "guest_ok=n"
            }
            .into(),
        ])?;
        Ok(())
    }
    fn delete(&self, name: &str) -> io::Result<()> {
        self.runner.run(vec!["delete".into(), name.into()])?;
        Ok(())
    }
    pub fn save(&self, original: Option<&Share>, draft: &Share) -> io::Result<Vec<Share>> {
        draft.validate()?;
        let shares = self.list()?;
        if let Some(old) = original {
            self.ensure_current(old, &shares)?;
            if draft.path != old.path || draft.guests != old.guests {
                return Err(invalid(
                    "A edição deve preservar a pasta e a configuração de convidados.",
                ));
            }
        }
        let renamed = original.is_some_and(|s| s.name.to_lowercase() != draft.name.to_lowercase());
        if (original.is_none() || renamed)
            && shares
                .iter()
                .any(|s| s.name.to_lowercase() == draft.name.to_lowercase())
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                crate::i18n::tr("Já existe um compartilhamento com esse nome."),
            ));
        }
        self.add(draft)?;
        if renamed {
            let old = original.expect("renamed existing share");
            // Create first: a name collision or invalid ACL never destroys the original.
            let mut created = None;
            let removal = self
                .list()
                .and_then(|current| {
                    created = current
                        .iter()
                        .find(|s| s.name.to_lowercase() == draft.name.to_lowercase())
                        .cloned();
                    self.ensure_current(old, &current)
                })
                .and_then(|()| self.delete(&old.name));
            if let Err(error) = removal {
                // Do not roll back a new share that somebody changed after creation.
                let cleanup = created
                    .as_ref()
                    .ok_or_else(|| {
                        io::Error::other(crate::i18n::tr(
                            "Atualize a lista para conferir os dois nomes.",
                        ))
                    })
                    .and_then(|new| {
                        self.list()
                            .and_then(|current| self.ensure_current(new, &current))
                            .and_then(|()| self.delete(&new.name))
                    });
                return Err(io::Error::other(crate::i18n::message(
                    "Não foi possível concluir a renomeação: {}. Limpeza do novo nome: {}.",
                    &[
                        error.to_string(),
                        cleanup
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| crate::i18n::tr("concluída").into()),
                    ],
                )));
            }
        }
        self.list()
    }
    pub fn stop(&self, original: &Share) -> io::Result<Vec<Share>> {
        validate_name(&original.name)?;
        self.ensure_current(original, &self.list()?)?;
        self.delete(&original.name)?;
        self.list()
    }
}

pub fn hosts() -> Vec<String> {
    let mut hosts = vec![];
    if let Ok(output) = Command::new("ip")
        .args(["-j", "-4", "address", "show", "scope", "global"])
        .stdin(Stdio::null())
        .output()
        && let Ok(interfaces) = serde_json::from_slice::<Vec<serde_json::Value>>(&output.stdout)
    {
        for interface in interfaces {
            let name = interface["ifname"].as_str().unwrap_or("");
            if !std::path::Path::new("/sys/class/net")
                .join(name)
                .join("device")
                .exists()
            {
                continue;
            }
            if let Some(addresses) = interface["addr_info"].as_array() {
                for address in addresses {
                    if let Some(ip) = address["local"]
                        .as_str()
                        .filter(|s| s.parse::<std::net::Ipv4Addr>().is_ok())
                    {
                        hosts.push(ip.into());
                    }
                }
            }
        }
    }
    if let Ok(host) = std::fs::read_to_string("/proc/sys/kernel/hostname") {
        let host = host.trim();
        if !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.'))
            && url::Url::parse(&format!("smb://{host}/")).is_ok()
            && !hosts.iter().any(|h| h == host)
        {
            hosts.push(host.into());
        }
    }
    if hosts.is_empty() {
        hosts.push("localhost".into());
    }
    hosts
}
#[cfg(test)]
mod tests;
