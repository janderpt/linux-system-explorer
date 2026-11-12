use super::*;
use std::{cell::RefCell, collections::BTreeSet};

#[derive(Default)]
struct Fake {
    shares: RefCell<Vec<Share>>,
    failed: RefCell<BTreeSet<(String, String)>>,
    mutations: RefCell<Vec<Vec<OsString>>>,
}
fn format_shares(shares: impl Iterator<Item = Share>) -> String {
    shares
        .map(|s| {
            format!(
                "[{}]\npath={}\ncomment={}\nusershare_acl={},\nguest_ok={}\n\n",
                s.name,
                s.path.display(),
                s.comment,
                s.acl(),
                if s.guests { "y" } else { "n" }
            )
        })
        .collect()
}
impl Runner for Fake {
    fn run(&self, args: Vec<OsString>) -> io::Result<String> {
        let verb = args[0].to_str().unwrap();
        if verb == "info" {
            let all = args.len() > 1;
            return Ok(format_shares(
                self.shares
                    .borrow()
                    .iter()
                    .filter(|s| all || s.editable)
                    .cloned(),
            ));
        }
        let name = args[1].to_str().unwrap();
        self.mutations.borrow_mut().push(args.clone());
        if self.failed.borrow().contains(&(verb.into(), name.into())) {
            return Err(io::Error::other("Injected Samba failure"));
        }
        let mut shares = self.shares.borrow_mut();
        shares.retain(|s| s.name.to_lowercase() != name.to_lowercase());
        if verb == "add" {
            let text = format!(
                "[{name}]\npath={}\ncomment={}\nusershare_acl={}\nguest_ok={}\n",
                args[2].to_string_lossy(),
                args[3].to_string_lossy(),
                args[4].to_string_lossy(),
                args[5].to_str().unwrap().trim_start_matches("guest_ok=")
            );
            let mut share = parse(&text)?.remove(0);
            share.editable = true;
            shares.push(share);
        }
        Ok(String::new())
    }
}
fn fixture(path: &std::path::Path, name: &str) -> Share {
    Share {
        name: name.into(),
        path: path.into(),
        comment: "Keep comment = intact".into(),
        rules: vec![
            Rule {
                principal: "PC\\jander".into(),
                access: Access::Write,
            },
            Rule {
                principal: "Everyone".into(),
                access: Access::Read,
            },
        ],
        guests: false,
        editable: true,
    }
}
#[test]
fn listing_preserves_acl_order_guests_unicode_and_ownership() {
    let root = tempfile::tempdir().unwrap();
    let one = fixture(root.path(), "Documentos públicos");
    let mut other = fixture(root.path(), "Other");
    other.editable = false;
    other.guests = true;
    other.rules.insert(
        0,
        Rule {
            principal: "S-1-5-21-42".into(),
            access: Access::Deny,
        },
    );
    let service = Service {
        runner: Fake::default(),
    };
    *service.runner.shares.borrow_mut() = vec![one.clone(), other.clone()];
    assert_eq!(service.list().unwrap(), vec![one, other]);
    assert!(parse("[bad]\npath=/tmp\nusershare_acl=Everyone:X\n").is_err());
    assert!(parse("[bad]\nusershare_acl=Everyone:R\n").is_err());
}
#[test]
fn edits_rename_permissions_and_stop_preserve_folder_contents() {
    let root = tempfile::tempdir().unwrap();
    let marker = root.path().join("keep.txt");
    std::fs::write(&marker, "keep").unwrap();
    let old = fixture(root.path(), "original");
    let service = Service {
        runner: Fake::default(),
    };
    service.runner.shares.borrow_mut().push(old.clone());
    let mut draft = old.clone();
    draft.name = "renamed".into();
    draft.rules[0].access = Access::Read;
    let listed = service.save(Some(&old), &draft).unwrap();
    assert_eq!(listed, vec![draft.clone()]);
    let mutations = service.runner.mutations.borrow();
    assert_eq!(mutations[0][0], "add");
    assert_eq!(mutations[1][0], "delete");
    drop(mutations);
    assert!(service.stop(&draft).unwrap().is_empty());
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "keep");
}
#[test]
fn collision_stale_state_and_other_owners_are_rejected_without_mutating() {
    let root = tempfile::tempdir().unwrap();
    let old = fixture(root.path(), "original");
    let service = Service {
        runner: Fake::default(),
    };
    let mut other = fixture(root.path(), "Taken");
    other.editable = false;
    *service.runner.shares.borrow_mut() = vec![old.clone(), other.clone()];
    let mut draft = old.clone();
    draft.name = "taken".into();
    assert_eq!(
        service.save(Some(&old), &draft).unwrap_err().kind(),
        io::ErrorKind::AlreadyExists
    );
    assert!(service.stop(&other).is_err());
    service.runner.shares.borrow_mut()[0].comment = "Changed externally".into();
    assert!(service.save(Some(&old), &old).is_err());
    assert!(service.stop(&old).is_err());
    assert!(service.runner.mutations.borrow().is_empty());
}
#[test]
fn failed_rename_restores_old_share_and_reports_failed_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let old = fixture(root.path(), "original");
    for cleanup_fails in [false, true] {
        let service = Service {
            runner: Fake::default(),
        };
        service.runner.shares.borrow_mut().push(old.clone());
        service
            .runner
            .failed
            .borrow_mut()
            .insert(("delete".into(), "original".into()));
        if cleanup_fails {
            service
                .runner
                .failed
                .borrow_mut()
                .insert(("delete".into(), "new".into()));
        }
        let mut draft = old.clone();
        draft.name = "new".into();
        let error = service.save(Some(&old), &draft).unwrap_err();
        let remaining = service.list().unwrap();
        assert!(remaining.contains(&old));
        assert_eq!(remaining.len(), if cleanup_fails { 2 } else { 1 });
        if cleanup_fails {
            assert!(error.to_string().contains("Injected Samba failure"));
        }
    }
}
#[test]
fn invalid_names_acl_injection_and_empty_permissions_never_reach_samba() {
    let root = tempfile::tempdir().unwrap();
    let service = Service {
        runner: Fake::default(),
    };
    for name in ["../escape", "-option", "wild*", "a\nb", "", ".."] {
        assert!(service.save(None, &fixture(root.path(), name)).is_err());
    }
    let mut draft = fixture(root.path(), "safe");
    draft.rules.clear();
    assert!(service.save(None, &draft).is_err());
    draft.rules = vec![Rule {
        principal: "Everyone:F,guest".into(),
        access: Access::Write,
    }];
    assert!(service.save(None, &draft).is_err());
    assert!(service.runner.mutations.borrow().is_empty());
    let mut draft = fixture(root.path(), "safe");
    draft.rules[0].principal = "name $(literal)".into();
    service.save(None, &draft).unwrap();
    let args = &service.runner.mutations.borrow()[0];
    assert_eq!(args[4], "name $(literal):F,Everyone:R");
    assert_eq!(args[5], "guest_ok=n");
}
#[test]
fn copy_addresses_use_the_saved_share_name_and_escape_uri_characters() {
    let root = tempfile::tempdir().unwrap();
    let share = fixture(root.path(), "Dados públicos #1");
    assert_eq!(
        share.address("192.0.2.30"),
        "smb://192.0.2.30/Dados%20p%C3%BAblicos%20%231"
    );
    assert_eq!(
        share.windows_address("192.0.2.30"),
        "\\\\192.0.2.30\\Dados públicos #1"
    );
}
#[test]
#[ignore = "Explicit host integration: creates and removes only uniquely named temporary Samba usershares"]
fn real_samba_user_share_lifecycle() {
    struct Cleanup {
        path: PathBuf,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let service = Service::default();
            if let Ok(shares) = service.list() {
                for share in shares
                    .into_iter()
                    .filter(|s| s.path == self.path && s.name.starts_with("lse-integration-"))
                {
                    let _ = service.stop(&share);
                }
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    let _cleanup = Cleanup {
        path: root.path().into(),
    };
    std::fs::write(root.path().join("keep.txt"), "keep").unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut draft = Share::new(root.path().into());
    draft.name = format!("lse-integration-{nonce}");
    let service = Service::default();
    let old = service
        .save(None, &draft)
        .unwrap()
        .into_iter()
        .find(|s| s.name == draft.name)
        .unwrap();
    let mut renamed = old.clone();
    renamed.name.push_str("-renamed");
    renamed.rules[0].access = Access::Read;
    let new = service
        .save(Some(&old), &renamed)
        .unwrap()
        .into_iter()
        .find(|s| s.name == renamed.name)
        .unwrap();
    assert_eq!(new.rules[0].access, Access::Read);
    assert!(
        !service
            .stop(&new)
            .unwrap()
            .iter()
            .any(|s| s.name == new.name)
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("keep.txt")).unwrap(),
        "keep"
    );
}

#[test]
fn explicit_denials_precede_broad_grants_when_saving_permissions() {
    let root = tempfile::tempdir().unwrap();
    let mut share = fixture(root.path(), "deny-order");
    share.rules = vec![
        Rule {
            principal: "Everyone".into(),
            access: Access::Read,
        },
        Rule {
            principal: "PC\\blocked".into(),
            access: Access::Deny,
        },
    ];
    let service = Service {
        runner: Fake::default(),
    };
    service.save(None, &share).unwrap();
    assert_eq!(
        service.runner.mutations.borrow()[0][4],
        "PC\\blocked:D,Everyone:R"
    );
}
