use std::{
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    str::FromStr,
};

use rand::random;

use super::{backup, restore};
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
