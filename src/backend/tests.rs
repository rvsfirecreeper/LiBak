use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
};

use rand::random;

use std::{collections::HashSet, ffi::OsStr};

use super::{
    backup, read_manifests, restore,
    types::{Manifest, SymlinkType, id_to_name},
    write_manifests,
};
#[test]
fn round_trip() {
    let fake_root_dir = tempfile::tempdir().unwrap();
    let fake_target_dir = tempfile::tempdir().unwrap();
    let mut fake_root_file = File::create(
        fake_root_dir
            .path()
            .join(PathBuf::from_str("rickroll").unwrap()),
    )
    .unwrap();
    fake_root_file.write_all(b"never gonna rickroll you up, never gonna let you down never gonna eat you alive also I love gd cologne").unwrap();
    fs::create_dir(
        fake_root_dir
            .path()
            .join(PathBuf::from_str("deeper_dir").unwrap()),
    )
    .unwrap();
    let mut deeper_file = File::create(
        fake_root_dir
            .path()
            .join(PathBuf::from_str("deeper_dir/deeper_file").unwrap()),
    )
    .unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            fake_root_dir
                .path()
                .join(PathBuf::from_str("deeper_dir/deeper_file").unwrap()),
            fake_root_dir
                .path()
                .join(PathBuf::from_str("deeper_dir/deeper_symlink").unwrap()),
        )
        .unwrap();
    }
    let _excluded_file = File::create(
        fake_root_dir
            .path()
            .join(PathBuf::from_str("deeper_dir/excluded_file").unwrap()),
    )
    .unwrap();
    deeper_file
        .write_all(b"infamous bermuda triangle method(elite ball knowledge needed)")
        .unwrap();
    let fake_store = tempfile::tempdir().unwrap();
    let key = crate::Key(random());
    let manifests = backup(
        fake_root_dir.path(),
        &key,
        &[&fake_root_dir
            .path()
            .join(PathBuf::from_str("deeper_dir/excluded_file").unwrap())],
        fake_store.path(),
    )
    .unwrap();
    restore(&manifests, fake_store.path(), fake_target_dir.path(), &key).unwrap();
    assert!(
        fs::exists(
            fake_target_dir
                .path()
                .join(PathBuf::from_str("rickroll").unwrap())
        )
        .unwrap(),
    );
    let mut data = Vec::new();
    File::open(
        fake_target_dir
            .path()
            .join(PathBuf::from_str("rickroll").unwrap()),
    )
    .unwrap()
    .read_to_end(&mut data)
    .unwrap();
    assert_eq!(data.as_slice(), b"never gonna rickroll you up, never gonna let you down never gonna eat you alive also I love gd cologne");
    assert!(
        fs::exists(
            fake_target_dir
                .path()
                .join(PathBuf::from_str("deeper_dir").unwrap())
        )
        .unwrap(),
    );
    assert!(
        fs::exists(
            fake_target_dir
                .path()
                .join(PathBuf::from_str("deeper_dir/deeper_file").unwrap())
        )
        .unwrap(),
    );
    assert!(
        fs::exists(
            fake_target_dir
                .path()
                .join(PathBuf::from_str("deeper_dir/deeper_symlink").unwrap())
        )
        .unwrap(),
    );
    assert!(
        !fs::exists(
            fake_target_dir
                .path()
                .join(PathBuf::from_str("deeper_dir/excluded_file").unwrap())
        )
        .unwrap(),
    );
    data = vec![];
    File::open(
        fake_target_dir
            .path()
            .join(PathBuf::from_str("deeper_dir/deeper_file").unwrap()),
    )
    .unwrap()
    .read_to_end(&mut data)
    .unwrap();
    assert_eq!(
        data.as_slice(),
        b"infamous bermuda triangle method(elite ball knowledge needed)"
    );
    #[cfg(unix)]
    {
        assert_eq!(
            fs::canonicalize(
                fake_target_dir
                    .path()
                    .join(PathBuf::from_str("deeper_dir/deeper_symlink").unwrap())
            )
            .unwrap(),
            fake_root_dir
                .path()
                .join(PathBuf::from_str("deeper_dir/deeper_file").unwrap()),
        );
    };
}

fn same_manifest(a: &Manifest, b: &Manifest) -> bool {
    match (a, b) {
        (
            Manifest::File {
                path: p1,
                chunks: c1,
            },
            Manifest::File {
                path: p2,
                chunks: c2,
            },
        ) => p1 == p2 && c1 == c2,
        (
            Manifest::Directory {
                path: p1,
                content: c1,
            },
            Manifest::Directory {
                path: p2,
                content: c2,
            },
        ) => p1 == p2 && c1 == c2,
        (
            Manifest::Symlink {
                path: p1,
                target: t1,
                ..
            },
            Manifest::Symlink {
                path: p2,
                target: t2,
                ..
            },
        ) => p1 == p2 && t1 == t2,
        _ => false,
    }
}

/// Linux only: `dirs` honours `XDG_DATA_HOME` there, so the test can point the
/// manifest store at a temp dir instead of the real data directory.
#[cfg(target_os = "linux")]
#[test]
fn manifest_round_trip() {
    let data_home = tempfile::tempdir().unwrap();
    // SAFETY: this is the only test that touches the environment.
    unsafe { std::env::set_var("XDG_DATA_HOME", data_home.path()) };

    let key = crate::Key(random());
    let backup_id: crate::BackupId = random();
    let manifests = vec![
        Manifest::Directory {
            path: PathBuf::new(),
            content: HashSet::from([PathBuf::from("a"), PathBuf::from("top")]),
        },
        Manifest::File {
            path: PathBuf::from("top"),
            chunks: vec![random(), random()],
        },
        Manifest::Directory {
            path: PathBuf::from("a"),
            content: HashSet::from([PathBuf::from("f"), PathBuf::from("l")]),
        },
        Manifest::File {
            path: PathBuf::from("a/f"),
            chunks: vec![random()],
        },
        Manifest::Symlink {
            path: PathBuf::from("a/l"),
            target: PathBuf::from("f"),
            symlink_type: SymlinkType::File,
        },
    ];

    write_manifests(&manifests, backup_id, &key).unwrap();
    let hex_id = id_to_name(&backup_id);
    let read = read_manifests(OsStr::new(&hex_id), &key).unwrap();
    assert_eq!(read.len(), manifests.len());
    for (written, read) in manifests.iter().zip(&read) {
        assert!(same_manifest(written, read));
    }

    // Wrong key must not decrypt.
    assert!(read_manifests(OsStr::new(&hex_id), &crate::Key(random())).is_err());

    // The index is authenticated: rewriting it must be detected.
    let dir = data_home.path().join("libak/manifests").join(&hex_id);
    let victim = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
    let mut bytes = fs::read(&victim).unwrap();
    bytes[0] ^= 0xff;
    fs::write(&victim, bytes).unwrap();
    assert!(read_manifests(OsStr::new(&hex_id), &key).is_err());
}
