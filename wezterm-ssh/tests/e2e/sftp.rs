use crate::sshd::session;
use assert_fs::{prelude::*, TempDir};
use predicates::prelude::*;
use rstest::*;
use ssh2::FileType;
use std::path::PathBuf;
use wezterm_ssh::Session;

// Sftp file tests
mod file;

#[inline]
fn file_type_to_str(file_type: FileType) -> &'static str {
    if file_type.is_dir() {
        "dir"
    } else if file_type.is_file() {
        "file"
    } else {
        "symlink"
    }
}

#[rstest]
#[smol_potat::test]
async fn readdir_should_return_list_of_directories_files_and_symlinks(#[future] session: Session) {
    let session: Session = session.await;

    // $TEMP/dir1/
    // $TEMP/dir2/
    // $TEMP/file1
    // $TEMP/file2
    // $TEMP/dir-link -> $TEMP/dir1/
    // $TEMP/file-link -> $TEMP/file1
    let temp = TempDir::new().unwrap();
    let dir1 = temp.child("dir1");
    dir1.create_dir_all().unwrap();
    let dir2 = temp.child("dir2");
    dir2.create_dir_all().unwrap();
    let file1 = temp.child("file1");
    file1.touch().unwrap();
    let file2 = temp.child("file2");
    file2.touch().unwrap();
    let link_dir = temp.child("link-dir");
    link_dir.symlink_to_dir(dir1.path()).unwrap();
    let link_file = temp.child("link-file");
    link_file.symlink_to_file(file1.path()).unwrap();

    let mut contents = session
        .sftp()
        .readdir(temp.path())
        .await
        .expect("Failed to read directory")
        .into_iter()
        .map(|(p, s)| (p, file_type_to_str(s.file_type())))
        .collect::<Vec<(PathBuf, &'static str)>>();
    contents.sort_unstable_by_key(|(p, _)| p.to_path_buf());

    assert_eq!(
        contents,
        vec![
            (dir1.path().to_path_buf(), "dir"),
            (dir2.path().to_path_buf(), "dir"),
            (file1.path().to_path_buf(), "file"),
            (file2.path().to_path_buf(), "file"),
            (link_dir.path().to_path_buf(), "symlink"),
            (link_file.path().to_path_buf(), "symlink"),
        ]
    );
}

#[rstest]
#[smol_potat::test]
async fn mkdir_should_create_a_directory_on_the_remote_filesystem(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    session
        .sftp()
        .mkdir(temp.child("dir").path(), 0o644)
        .await
        .expect("Failed to create directory");

    // Verify the path exists and is to a directory
    temp.child("dir").assert(predicate::path::is_dir());
}

#[rstest]
#[smol_potat::test]
async fn mkdir_should_return_error_if_unable_to_create_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    // Attempt to create a nested directory structure, which is not supported
    let result = session
        .sftp()
        .mkdir(temp.child("dir").child("dir").path(), 0o644)
        .await;
    assert!(
        result.is_err(),
        "Unexpectedly succeeded in creating directory: {:?}",
        result
    );

    // Verify the path is not a directory
    temp.child("dir")
        .child("dir")
        .assert(predicate::path::is_dir().not());
    temp.child("dir").assert(predicate::path::is_dir().not());
}

#[rstest]
#[smol_potat::test]
async fn rmdir_should_remove_a_remote_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    // Removing an empty directory should succeed
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    session
        .sftp()
        .rmdir(dir.path())
        .await
        .expect("Failed to remove directory");

    // Verify the directory no longer exists
    dir.assert(predicate::path::is_dir().not());
}

#[rstest]
#[smol_potat::test]
async fn rmdir_should_return_an_error_if_failed_to_remove_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    // Attempt to remove a missing path
    let result = session.sftp().rmdir(temp.child("missing-dir").path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly succeeded in removing missing directory: {:?}",
        result
    );

    // Attempt to remove a non-empty directory
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    dir.child("file").touch().unwrap();

    let result = session.sftp().rmdir(dir.path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly succeeded in removing non-empty directory: {:?}",
        result
    );

    // Verify the non-empty directory still exists
    dir.assert(predicate::path::is_dir());

    // Attempt to remove a file (not a directory)
    let file = temp.child("file");
    file.touch().unwrap();
    let result = session.sftp().rmdir(file.path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly succeeded in removing file: {:?}",
        result
    );

    // Verify the file still exists
    file.assert(predicate::path::is_file());
}

#[rstest]
#[smol_potat::test]
async fn stat_should_return_metadata_about_a_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");
    file.touch().unwrap();

    let stat = session
        .sftp()
        .stat(file.path())
        .await
        .expect("Failed to stat file");

    // Verify that file stat makes sense
    assert!(stat.is_file(), "Invalid file stat returned");
}

#[rstest]
#[smol_potat::test]
async fn stat_should_return_metadata_about_a_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();

    let stat = session
        .sftp()
        .stat(dir.path())
        .await
        .expect("Failed to stat dir");

    // Verify that file stat makes sense
    assert!(stat.is_dir(), "Invalid file stat returned");
}

#[rstest]
#[smol_potat::test]
async fn stat_should_return_metadata_about_the_file_pointed_to_by_a_symlink(
    #[future] session: Session,
) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let file = temp.child("file");
    file.touch().unwrap();
    let link = temp.child("link");
    link.symlink_to_file(file.path()).unwrap();

    let stat = session
        .sftp()
        .stat(link.path())
        .await
        .expect("Failed to stat symlink");

    // Verify that file stat makes sense
    assert!(stat.is_file(), "Invalid file stat returned");
    assert!(stat.file_type().is_file(), "Invalid file stat returned");
    assert!(!stat.file_type().is_symlink(), "Invalid file stat returned");
}

#[rstest]
#[smol_potat::test]
async fn stat_should_return_metadata_about_the_dir_pointed_to_by_a_symlink(
    #[future] session: Session,
) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let link = temp.child("link");
    link.symlink_to_dir(dir.path()).unwrap();

    let stat = session
        .sftp()
        .stat(link.path())
        .await
        .expect("Failed to stat symlink");

    // Verify that file stat makes sense
    assert!(stat.is_dir(), "Invalid file stat returned");
    assert!(stat.file_type().is_dir(), "Invalid file stat returned");
    assert!(!stat.file_type().is_symlink(), "Invalid file stat returned");
}

#[rstest]
#[smol_potat::test]
async fn stat_should_fail_if_path_missing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let result = session.sftp().stat(temp.child("missing").path()).await;
    assert!(result.is_err(), "Stat unexpectedly succeeded: {:?}", result);
}

#[rstest]
#[smol_potat::test]
async fn lstat_should_return_metadata_about_a_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");
    file.touch().unwrap();

    let lstat = session
        .sftp()
        .lstat(file.path())
        .await
        .expect("Failed to lstat file");

    // Verify that file lstat makes sense
    assert!(lstat.is_file(), "Invalid file lstat returned");
}

#[rstest]
#[smol_potat::test]
async fn lstat_should_return_metadata_about_a_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();

    let lstat = session
        .sftp()
        .lstat(dir.path())
        .await
        .expect("Failed to lstat dir");

    // Verify that file lstat makes sense
    assert!(lstat.is_dir(), "Invalid file lstat returned");
}

#[rstest]
#[smol_potat::test]
async fn lstat_should_return_metadata_about_symlink_pointing_to_a_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let file = temp.child("file");
    file.touch().unwrap();
    let link = temp.child("link");
    link.symlink_to_file(file.path()).unwrap();

    let lstat = session
        .sftp()
        .lstat(link.path())
        .await
        .expect("Failed to lstat symlink");

    // Verify that file lstat makes sense
    assert!(!lstat.is_file(), "Invalid file lstat returned");
    assert!(!lstat.file_type().is_file(), "Invalid file lstat returned");
    assert!(
        lstat.file_type().is_symlink(),
        "Invalid file lstat returned"
    );
}

#[rstest]
#[smol_potat::test]
async fn lstat_should_return_metadata_about_symlink_pointing_to_a_directory(
    #[future] session: Session,
) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let link = temp.child("link");
    link.symlink_to_dir(dir.path()).unwrap();

    let lstat = session
        .sftp()
        .lstat(link.path())
        .await
        .expect("Failed to lstat symlink");

    // Verify that file lstat makes sense
    assert!(!lstat.is_dir(), "Invalid file lstat returned");
    assert!(!lstat.file_type().is_dir(), "Invalid file lstat returned");
    assert!(
        lstat.file_type().is_symlink(),
        "Invalid file lstat returned"
    );
}

#[rstest]
#[smol_potat::test]
async fn lstat_should_fail_if_path_missing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let result = session.sftp().lstat(temp.child("missing").path()).await;
    assert!(
        result.is_err(),
        "lstat unexpectedly succeeded: {:?}",
        result
    );
}

#[rstest]
#[smol_potat::test]
async fn symlink_should_create_symlink_pointing_to_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");
    file.touch().unwrap();

    let link = temp.child("link");

    session
        .sftp()
        .symlink(file.path(), link.path())
        .await
        .expect("Failed to create symlink");

    assert!(
        std::fs::symlink_metadata(link.path())
            .unwrap()
            .file_type()
            .is_symlink(),
        "Symlink is not a symlink!"
    );

    // TODO: This fails even though the type is a symlink:
    //       https://github.com/assert-rs/assert_fs/issues/70
    // link.assert(predicate::path::is_symlink());
}

#[rstest]
#[smol_potat::test]
async fn symlink_should_create_symlink_pointing_to_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();

    let link = temp.child("link");

    session
        .sftp()
        .symlink(dir.path(), link.path())
        .await
        .expect("Failed to create symlink");

    link.assert(predicate::path::is_symlink());
}

#[rstest]
#[smol_potat::test]
async fn symlink_should_succeed_even_if_path_missing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");

    let link = temp.child("link");

    session
        .sftp()
        .symlink(file.path(), link.path())
        .await
        .expect("Failed to create symlink");

    link.assert(predicate::path::is_symlink());
}

#[rstest]
#[smol_potat::test]
async fn readlink_should_return_the_target_of_the_symlink(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    // Test a symlink to a directory
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let link = temp.child("link");
    link.symlink_to_dir(dir.path()).unwrap();

    let path = session
        .sftp()
        .readlink(link.path())
        .await
        .expect("Failed to read symlink");
    assert_eq!(path, dir.path());

    // Test a symlink to a file
    let file = temp.child("file");
    file.touch().unwrap();
    let link = temp.child("link2");
    link.symlink_to_file(file.path()).unwrap();

    let path = session
        .sftp()
        .readlink(link.path())
        .await
        .expect("Failed to read symlink");
    assert_eq!(path, file.path());
}

#[rstest]
#[smol_potat::test]
async fn readlink_should_fail_if_path_is_not_a_symlink(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    // Test missing path
    let result = session.sftp().readlink(temp.child("missing").path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly read link for missing path: {:?}",
        result
    );

    // Test a directory
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let result = session.sftp().readlink(dir.path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly read link for directory: {:?}",
        result
    );

    // Test a file
    let file = temp.child("file");
    file.touch().unwrap();
    let result = session.sftp().readlink(file.path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly read link for file: {:?}",
        result
    );
}

#[rstest]
#[smol_potat::test]
async fn realpath_should_resolve_absolute_path_for_relative_path(#[future] session: Session) {
    let session: Session = session.await;

    // For resolving parts of a path, all components must exist
    let temp = TempDir::new().unwrap();
    temp.child("hello").create_dir_all().unwrap();
    temp.child("world").touch().unwrap();

    let rel = temp.child(".").child("hello").child("..").child("world");

    // NOTE: Because realpath can still resolve symlinks within a missing path, there
    //       is no guarantee that the resulting path matches the missing path. In fact,
    //       on mac the /tmp dir is a symlink to /private/tmp; so, we cannot successfully
    //       check the accuracy of the path itself, meaning that we can only validate
    //       that the operation was okay.
    let result = session.sftp().realpath(rel.path()).await;
    assert!(result.is_ok(), "Realpath unexpectedly failed: {:?}", result);
}

#[rstest]
#[smol_potat::test]
async fn realpath_should_return_resolved_path_if_missing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let missing = temp.child("missing");

    // NOTE: Because realpath can still resolve symlinks within a missing path, there
    //       is no guarantee that the resulting path matches the missing path. In fact,
    //       on mac the /tmp dir is a symlink to /private/tmp; so, we cannot successfully
    //       check the accuracy of the path itself, meaning that we can only validate
    //       that the operation was okay.
    let result = session.sftp().realpath(missing.path()).await;
    assert!(result.is_ok(), "Realpath unexpectedly failed: {:?}", result);
}

#[rstest]
#[smol_potat::test]
async fn realpath_should_fail_if_resolving_missing_path_with_dots(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let missing = temp.child(".").child("hello").child("..").child("world");

    let result = session.sftp().realpath(missing.path()).await;
    assert!(result.is_err(), "Realpath unexpectedly succeeded");
}

#[rstest]
#[smol_potat::test]
async fn rename_should_support_singular_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");
    file.write_str("some text").unwrap();

    let dst = temp.child("dst");

    session
        .sftp()
        .rename(file.path(), dst.path(), None)
        .await
        .expect("Failed to rename file");

    // Verify that file was moved to destination
    file.assert(predicate::path::missing());
    dst.assert("some text");
}

#[rstest]
#[smol_potat::test]
async fn rename_should_support_dirtectory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let dir_file = dir.child("file");
    dir_file.write_str("some text").unwrap();
    let dir_dir = dir.child("dir");
    dir_dir.create_dir_all().unwrap();

    let dst = temp.child("dst");

    session
        .sftp()
        .rename(dir.path(), dst.path(), None)
        .await
        .expect("Failed to rename directory");

    // Verify that directory was moved to destination
    dir.assert(predicate::path::missing());
    dir_file.assert(predicate::path::missing());
    dir_dir.assert(predicate::path::missing());

    dst.assert(predicate::path::is_dir());
    dst.child("file").assert("some text");
    dst.child("dir").assert(predicate::path::is_dir());
}

#[rstest]
#[smol_potat::test]
async fn rename_should_fail_if_source_path_missing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let missing = temp.child("missing");
    let dst = temp.child("dst");

    let result = session
        .sftp()
        .rename(missing.path(), dst.path(), None)
        .await;
    assert!(
        result.is_err(),
        "Rename unexpectedly succeeded with missing path: {:?}",
        result
    );
}

#[rstest]
#[smol_potat::test]
async fn unlink_should_remove_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");
    file.touch().unwrap();

    session
        .sftp()
        .unlink(file.path())
        .await
        .expect("Failed to unlink file");

    file.assert(predicate::path::missing());
}

#[rstest]
#[smol_potat::test]
async fn unlink_should_remove_symlink_to_file(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("file");
    file.touch().unwrap();
    let link = temp.child("link");
    link.symlink_to_file(file.path()).unwrap();

    session
        .sftp()
        .unlink(link.path())
        .await
        .expect("Failed to unlink symlink");

    // Verify link removed but file still exists
    link.assert(predicate::path::missing());
    file.assert(predicate::path::is_file());
}

#[rstest]
#[smol_potat::test]
async fn unlink_should_remove_symlink_to_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let link = temp.child("link");
    link.symlink_to_dir(dir.path()).unwrap();

    session
        .sftp()
        .unlink(link.path())
        .await
        .expect("Failed to unlink symlink");

    // Verify link removed but directory still exists
    link.assert(predicate::path::missing());
    dir.assert(predicate::path::is_dir());
}

#[rstest]
#[smol_potat::test]
async fn unlink_should_fail_if_path_to_directory(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();

    let result = session.sftp().unlink(dir.path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly unlinked directory: {:?}",
        result
    );

    // Verify directory still here
    dir.assert(predicate::path::is_dir());
}

#[rstest]
#[smol_potat::test]
async fn unlink_should_fail_if_path_missing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();

    let result = session.sftp().unlink(temp.child("missing").path()).await;
    assert!(
        result.is_err(),
        "Unexpectedly unlinked missing path: {:?}",
        result
    );
}
