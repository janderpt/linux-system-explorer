use super::*;
use std::sync::{Arc, Mutex, mpsc};
use zbus::OwnedObjectPath;
fn value<'a>(v: impl Into<Variant<'a>>) -> OwnedVariant {
    OwnedVariant::try_from(v.into()).unwrap()
}
fn fixture() -> ManagedObjects {
    let mut objects = ManagedObjects::new();
    let drive = format!("{ROOT}/drives/usb");
    objects.insert(
        OwnedObjectPath::try_from(drive.clone()).unwrap(),
        HashMap::from([(
            zbus::names::OwnedInterfaceName::try_from(DRIVE).unwrap(),
            HashMap::from([
                ("Removable".into(), value(true)),
                ("CanPowerOff".into(), value(true)),
                ("Id".into(), value("USB-Test")),
                ("Serial".into(), value("test123")),
            ]),
        )]),
    );
    for (name, mount) in [("sdb1", Some("/media/test")), ("sdb2", None)] {
        let block = HashMap::from([
            (
                "Device".into(),
                value(format!("/dev/{name}\0").into_bytes()),
            ),
            (
                "Drive".into(),
                value(zbus::ObjectPath::try_from(drive.as_str()).unwrap()),
            ),
            ("IdUsage".into(), value("filesystem")),
            ("IdType".into(), value("ext4")),
            ("IdLabel".into(), value(name)),
            ("IdUUID".into(), value(name)),
            ("Size".into(), value(1024u64)),
        ]);
        let mounts: Vec<Vec<u8>> = mount
            .map(|m| vec![format!("{m}\0").into_bytes()])
            .unwrap_or_default();
        let fs = HashMap::from([("MountPoints".into(), value(mounts))]);
        objects.insert(
            OwnedObjectPath::try_from(format!("{ROOT}/block_devices/{name}")).unwrap(),
            HashMap::from([
                (
                    zbus::names::OwnedInterfaceName::try_from(BLOCK).unwrap(),
                    block,
                ),
                (
                    zbus::names::OwnedInterfaceName::try_from(FILESYSTEM).unwrap(),
                    fs,
                ),
            ]),
        );
    }
    objects
}
#[test]
fn parses_visible_internal_removable_unmounted_and_raw_paths_and_protects_system_partitions() {
    let mut objects = fixture();
    let key = OwnedObjectPath::try_from(format!("{ROOT}/block_devices/sdb1")).unwrap();
    let mut volumes = parse(&objects);
    assert_eq!(volumes.len(), 2);
    assert_eq!(volumes[0].mount(), Some(Path::new("/media/test")));
    assert!(volumes[0].can_unmount() && volumes[0].can_eject());
    assert!(volumes[1].mount().is_none());
    assert!(volumes[1].capacity().contains("Desmontado"));
    let interfaces = objects.get_mut(&key).unwrap();
    interfaces
        .get_mut(FILESYSTEM)
        .unwrap()
        .insert("MountPoints".into(), value(vec![b"/home\0".to_vec()]));
    interfaces
        .get_mut(BLOCK)
        .unwrap()
        .insert("HintIgnore".into(), value(true));
    volumes = parse(&objects);
    assert!(volumes[0].protected && !volumes[0].visible && !volumes[0].can_unmount());
    assert!(
        volumes.iter().all(|v| !v.can_eject()),
        "hidden system partition protects whole drive"
    );
    let interfaces = objects.get_mut(&key).unwrap();
    interfaces.get_mut(FILESYSTEM).unwrap().insert(
        "MountPoints".into(),
        value(vec![
            b"/media/raw-\xff\0".to_vec(),
            b"relative\0".to_vec(),
            b"/bad\0path\0".to_vec(),
        ]),
    );
    volumes = parse(&objects);
    assert_eq!(volumes[0].mounts.len(), 1);
    assert_eq!(
        volumes[0].mount().unwrap(),
        Path::new(&OsString::from_vec(b"/media/raw-\xff".to_vec()))
    );
    objects.get_mut(&key).unwrap().insert(
        zbus::names::OwnedInterfaceName::try_from("org.freedesktop.UDisks2.Loop").unwrap(),
        HashMap::new(),
    );
    assert_eq!(parse(&objects).len(), 1, "exclude Snap/loop filesystems");
    let usage = space(Path::new("/")).unwrap();
    assert!(usage.total > 0 && usage.available <= usage.total);
}
struct Manager(Arc<Mutex<ManagedObjects>>);
#[zbus::interface(name = "org.freedesktop.DBus.ObjectManager")]
impl Manager {
    fn get_managed_objects(&self) -> ManagedObjects {
        let objects = self.0.lock().unwrap();
        objects
            .iter()
            .map(|(p, i)| {
                (
                    p.clone(),
                    i.iter()
                        .map(|(n, props)| {
                            (
                                n.clone(),
                                props
                                    .iter()
                                    .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
                                    .collect(),
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }
}
struct Filesystem {
    id: String,
    calls: Arc<Mutex<Vec<String>>>,
    fail: Arc<Mutex<bool>>,
}
#[zbus::interface(name = "org.freedesktop.UDisks2.Filesystem")]
impl Filesystem {
    fn mount(
        &self,
        options: HashMap<String, OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        assert!(
            header
                .primary()
                .flags()
                .contains(zbus::message::Flags::ALLOW_INTERACTIVE_AUTH)
        );
        assert!(options.is_empty());
        self.calls
            .lock()
            .unwrap()
            .push(format!("Mount:{}", self.id));
        if *self.fail.lock().unwrap() {
            return Err(zbus::fdo::Error::AccessDenied("Denied".into()));
        }
        Ok("/media/with space".into())
    }
    fn unmount(
        &self,
        options: HashMap<String, OwnedVariant>,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<()> {
        assert!(
            header
                .primary()
                .flags()
                .contains(zbus::message::Flags::ALLOW_INTERACTIVE_AUTH)
        );
        assert!(
            options.is_empty(),
            "never force or disable interactive authorization"
        );
        self.calls
            .lock()
            .unwrap()
            .push(format!("Unmount:{}", self.id));
        if *self.fail.lock().unwrap() {
            return Err(zbus::fdo::Error::Failed("Device busy".into()));
        }
        Ok(())
    }
}
struct Drive(Arc<Mutex<Vec<String>>>);
#[zbus::interface(name = "org.freedesktop.UDisks2.Drive")]
impl Drive {
    fn power_off(&self, options: HashMap<String, OwnedVariant>) {
        assert!(options.is_empty());
        self.0.lock().unwrap().push("PowerOff".into());
    }
    fn eject(&self, options: HashMap<String, OwnedVariant>) {
        assert!(options.is_empty());
        self.0.lock().unwrap().push("Eject".into());
    }
}
#[test]
fn mount_open_unmount_eject_revalidate_and_stop_on_busy_or_denial_on_a_private_bus() {
    if std::env::var_os("LSE_VOLUMES_PRIVATE_BUS").is_none() {
        let status=std::process::Command::new("dbus-run-session").arg("--").arg(std::env::current_exe().unwrap()).args(["--exact","volumes::tests::mount_open_unmount_eject_revalidate_and_stop_on_busy_or_denial_on_a_private_bus","--nocapture"]).env("LSE_VOLUMES_PRIVATE_BUS","1").status().unwrap();
        assert!(status.success());
        return;
    }
    let objects = Arc::new(Mutex::new(fixture()));
    let calls = Arc::new(Mutex::new(vec![]));
    let fail = Arc::new(Mutex::new(false));
    let (ready_tx, ready) = mpsc::channel();
    let (stop, stopped) = mpsc::channel();
    let o = objects.clone();
    let c = calls.clone();
    let f = fail.clone();
    let worker = std::thread::spawn(move || {
        let connection = zbus::block_on(async {
            let connection = zbus::connection::Builder::session()
                .name(DESTINATION)
                .serve_at(ROOT, Manager(o))
                .build()
                .await?;
            for id in ["sdb1", "sdb2"] {
                connection
                    .object_server()
                    .at(
                        format!("{ROOT}/block_devices/{id}"),
                        Filesystem {
                            id: id.into(),
                            calls: c.clone(),
                            fail: f.clone(),
                        },
                    )
                    .await?;
            }
            connection
                .object_server()
                .at(format!("{ROOT}/drives/usb"), Drive(c))
                .await?;
            Ok::<_, zbus::Error>(connection)
        })
        .unwrap();
        ready_tx.send(()).unwrap();
        stopped.recv().unwrap();
        drop(connection);
    });
    ready.recv().unwrap();
    zbus::block_on(async {
        let connection = zbus::Connection::session().await.unwrap();
        let selected = parse(&objects.lock().unwrap());
        assert_eq!(
            perform(&connection, DESTINATION, &selected[0], Action::Open)
                .await
                .unwrap(),
            Outcome::Opened("/media/test".into())
        );
        assert!(
            calls.lock().unwrap().is_empty(),
            "opening mounted volume does not mount again"
        );
        assert_eq!(
            perform(&connection, DESTINATION, &selected[1], Action::Open)
                .await
                .unwrap(),
            Outcome::Opened("/media/with space".into())
        );
        assert_eq!(
            perform(&connection, DESTINATION, &selected[0], Action::Unmount)
                .await
                .unwrap(),
            Outcome::Unmounted(vec!["/media/test".into()])
        );
        calls.lock().unwrap().clear();
        *fail.lock().unwrap() = true;
        assert!(
            perform(&connection, DESTINATION, &selected[1], Action::Open)
                .await
                .is_err()
        );
        assert!(
            perform(&connection, DESTINATION, &selected[0], Action::Eject)
                .await
                .is_err()
        );
        assert!(
            !calls.lock().unwrap().contains(&"PowerOff".into()),
            "busy volume prevents drive removal"
        );
        *fail.lock().unwrap() = false;
        calls.lock().unwrap().clear();
        assert_eq!(
            perform(&connection, DESTINATION, &selected[0], Action::Eject)
                .await
                .unwrap(),
            Outcome::Ejected(vec!["/media/test".into()])
        );
        assert_eq!(*calls.lock().unwrap(), vec!["Unmount:sdb1", "PowerOff"]);
        let drive = OwnedObjectPath::try_from(format!("{ROOT}/drives/usb")).unwrap();
        {
            let mut objects = objects.lock().unwrap();
            let drive = objects.get_mut(&drive).unwrap().get_mut(DRIVE).unwrap();
            drive.insert("CanPowerOff".into(), value(true));
            drive.insert("Optical".into(), value(true));
            drive.insert("Ejectable".into(), value(true));
        }
        calls.lock().unwrap().clear();
        assert!(matches!(
            perform(&connection, DESTINATION, &selected[0], Action::Eject)
                .await
                .unwrap(),
            Outcome::Ejected(_)
        ));
        assert_eq!(*calls.lock().unwrap(), vec!["Unmount:sdb1", "Eject"]);
        let key = OwnedObjectPath::try_from(selected[1].id.clone()).unwrap();
        objects
            .lock()
            .unwrap()
            .get_mut(&key)
            .unwrap()
            .get_mut(BLOCK)
            .unwrap()
            .insert("IdUUID".into(), value("replaced-device"));
        calls.lock().unwrap().clear();
        assert!(
            perform(&connection, DESTINATION, &selected[1], Action::Open)
                .await
                .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
        let key = OwnedObjectPath::try_from(selected[0].id.clone()).unwrap();
        objects
            .lock()
            .unwrap()
            .get_mut(&key)
            .unwrap()
            .get_mut(FILESYSTEM)
            .unwrap()
            .insert("MountPoints".into(), value(vec![b"/\0".to_vec()]));
        assert!(
            perform(&connection, DESTINATION, &selected[0], Action::Unmount)
                .await
                .is_err()
        );
        assert!(
            perform(&connection, DESTINATION, &selected[0], Action::Eject)
                .await
                .is_err()
        );
        assert!(calls.lock().unwrap().is_empty());
    });
    stop.send(()).unwrap();
    worker.join().unwrap();
}
#[test]
#[ignore = "read-only check of the host UDisks2 service; no mounting or ejection"]
fn native_volume_inventory_is_read_only() {
    let volumes = list().unwrap();
    for v in volumes {
        println!(
            "{} · {} · {:?} · {}",
            v.name,
            v.device.display(),
            v.mounts,
            v.capacity()
        );
    }
}
