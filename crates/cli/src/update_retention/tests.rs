//! Offline updater integration tests using a local release source and HTTP fixture server.

use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use self_update::{Release, ReleaseAsset, ReleaseSource};
use tempfile::TempDir;

use super::{ArchiveRetention, release_directory, validate_archive_name};

const NEW_BINARY: &[u8] = b"new executable bytes\n";
const MAC_NAME: &str = "oatty-v1.1.0-aarch64-apple-darwin.zip";
const MAC_ZIP: &[u8] = include_bytes!("fixtures/macos.zip");

struct FixtureServer {
    url: String,
    requests: Arc<AtomicUsize>,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl FixtureServer {
    fn start(payload: &'static [u8]) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("loopback listener");
        listener.set_nonblocking(true).expect("nonblocking listener");
        let url = format!("http://{}/archive", listener.local_addr().expect("address"));
        let requests = Arc::new(AtomicUsize::new(0));
        let stopped = Arc::new(AtomicBool::new(false));
        let worker_requests = Arc::clone(&requests);
        let worker_stopped = Arc::clone(&stopped);
        let worker = thread::spawn(move || {
            while !worker_stopped.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut connection, _)) => {
                        connection.set_read_timeout(Some(Duration::from_secs(5))).expect("read timeout");
                        let mut request = Vec::new();
                        let mut buffer = [0; 1024];
                        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                            let length = connection.read(&mut buffer).expect("request headers");
                            assert!(length > 0 && request.len() < 16384, "bounded HTTP request");
                            request.extend_from_slice(&buffer[..length]);
                        }
                        write!(
                            connection,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            payload.len()
                        )
                        .expect("response headers");
                        connection.write_all(payload).expect("response archive");
                        worker_requests.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(2)),
                    Err(error) => panic!("loopback accept: {error}"),
                }
            }
        });
        Self {
            url,
            requests,
            stopped,
            worker: Some(worker),
        }
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.worker.take().expect("server worker").join().expect("server completed");
    }
}

struct FixtureSource(Release);

impl ReleaseSource for FixtureSource {
    fn get_releases(&self) -> self_update::Result<Vec<Release>> {
        Ok(vec![self.0.clone()])
    }
}

fn updater(
    server: &FixtureServer,
    name: &str,
    target: &str,
    installed: &Path,
    current_version: &str,
    retention: ArchiveRetention,
) -> self_update::backends::custom::Update {
    let binary_name = if target.ends_with("windows-msvc") { "oatty.exe" } else { "oatty" };
    let release = Release::builder()
        .version("1.1.0")
        .asset(ReleaseAsset::new(name, server.url.clone()))
        .build()
        .expect("fixture release");
    self_update::backends::custom::Update::configure()
        .source(FixtureSource(release))
        .target(target)
        .bin_name(binary_name)
        .bin_path_in_archive(binary_name)
        .bin_install_path(installed)
        .current_version(current_version)
        .show_output(false)
        .no_confirm(true)
        .timeout(Duration::from_secs(5))
        .reqwest_client(reqwest::blocking::Client::builder().no_proxy().build().expect("loopback client"))
        .verify_archive(move |archive| retention.verify(archive))
        .build()
        .expect("fixture updater")
}

#[test]
fn complete_compressed_archives_survive_updater_cleanup_and_replace_only_test_binary() {
    let fixtures = [
        (MAC_NAME, "aarch64-apple-darwin", MAC_ZIP),
        (
            "oatty-v1.1.0-x86_64-pc-windows-msvc.zip",
            "x86_64-pc-windows-msvc",
            include_bytes!("fixtures/windows.zip").as_slice(),
        ),
        (
            "oatty-v1.1.0-x86_64-unknown-linux-gnu.tar.gz",
            "x86_64-unknown-linux-gnu",
            include_bytes!("fixtures/linux.tar.gz").as_slice(),
        ),
    ];
    for (name, target, archive) in fixtures {
        let temporary = TempDir::new().expect("temporary directory");
        let server = FixtureServer::start(archive);
        let installed = temporary.path().join("installed");
        fs::write(&installed, b"old binary").expect("old executable");
        let root = temporary.path().join("releases");
        let retention = ArchiveRetention::in_directory(root.clone());
        let updater = updater(&server, name, target, &installed, "1.0.0", retention.clone());
        assert!(!root.exists(), "building an updater must not resolve/create storage");
        retention.complete(updater.update()).expect("offline update succeeds");
        drop(updater);
        let saved = retention.saved_path().expect("retention state").expect("saved archive");
        assert_eq!(saved.file_name().expect("original filename"), name);
        assert!(saved.parent().expect("attempt directory").starts_with(&root));
        assert_eq!(fs::read(saved).expect("retained archive"), archive);
        assert_eq!(fs::read(installed).expect("installed executable"), NEW_BINARY);
        assert_eq!(server.requests.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn retention_uses_actual_download_name_when_release_changes_after_checking() {
    struct ChangingSource {
        first: Release,
        subsequent: Release,
        calls: AtomicUsize,
    }
    impl ReleaseSource for ChangingSource {
        fn get_releases(&self) -> self_update::Result<Vec<Release>> {
            Ok(vec![if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.first.clone()
            } else {
                self.subsequent.clone()
            }])
        }
    }
    let temporary = TempDir::new().expect("temporary directory");
    let server = FixtureServer::start(MAC_ZIP);
    let installed = temporary.path().join("installed");
    fs::write(&installed, b"old binary").expect("old executable");
    let retention = ArchiveRetention::in_directory(temporary.path().join("releases"));
    let callback = retention.clone();
    let actual_name = "oatty-v1.2.0-aarch64-apple-darwin.zip";
    let updater = self_update::backends::custom::Update::configure()
        .source(ChangingSource {
            first: Release::builder()
                .version("1.1.0")
                .asset(ReleaseAsset::new(MAC_NAME, server.url.clone()))
                .build()
                .expect("first release"),
            subsequent: Release::builder()
                .version("1.2.0")
                .asset(ReleaseAsset::new(actual_name, server.url.clone()))
                .build()
                .expect("new release"),
            calls: AtomicUsize::new(0),
        })
        .target("aarch64-apple-darwin")
        .bin_name("oatty")
        .bin_install_path(&installed)
        .current_version("1.0.0")
        .show_output(false)
        .no_confirm(true)
        .reqwest_client(reqwest::blocking::Client::builder().no_proxy().build().expect("loopback client"))
        .verify_archive(move |archive| callback.verify(archive))
        .build()
        .expect("fixture updater");
    assert_eq!(
        updater
            .get_latest_release()
            .expect("preflight check")
            .latest()
            .expect("release")
            .version(),
        "1.1.0"
    );
    retention.complete(updater.update()).expect("actual update");
    let saved = retention.saved_path().expect("state").expect("saved archive");
    assert_eq!(saved.file_name().expect("actual name"), actual_name);
    assert_eq!(fs::read(saved).expect("original archive bytes"), MAC_ZIP);
}

#[test]
fn retention_failure_rejects_installation_without_replacing_binary() {
    let temporary = TempDir::new().expect("temporary directory");
    let root = temporary.path().join("storage-is-a-file");
    fs::write(&root, b"not a directory").expect("deterministic write failure");
    let installed = temporary.path().join("installed");
    fs::write(&installed, b"old binary").expect("old executable");
    let retention = ArchiveRetention::in_directory(root);
    let server = FixtureServer::start(MAC_ZIP);
    let updater = updater(&server, MAC_NAME, "aarch64-apple-darwin", &installed, "1.0.0", retention.clone());
    let error = retention.complete(updater.update()).expect_err("retention must reject update");
    assert!(format!("{error:#}").contains("Could not save release archive"));
    assert_eq!(fs::read(installed).expect("original executable"), b"old binary");
    assert!(retention.saved_path().expect("state").is_none());
}

#[test]
fn later_install_failure_keeps_archive_and_reports_its_path() {
    let temporary = TempDir::new().expect("temporary directory");
    let blocker = temporary.path().join("install-parent-is-a-file");
    fs::write(&blocker, b"unchanged blocker").expect("deterministic installation failure");
    let installed = blocker.join("installed");
    let retention = ArchiveRetention::in_directory(temporary.path().join("releases"));
    let server = FixtureServer::start(MAC_ZIP);
    let updater = updater(&server, MAC_NAME, "aarch64-apple-darwin", &installed, "1.0.0", retention.clone());
    let error = retention.complete(updater.update()).expect_err("install must fail");
    let saved = retention.saved_path().expect("state").expect("saved before installation failure");
    assert!(format!("{error:#}").contains(saved.to_str().expect("temporary UTF-8 path")));
    assert_eq!(fs::read(saved).expect("preserved archive"), MAC_ZIP);
    assert_eq!(fs::read(blocker).expect("unmodified destination"), b"unchanged blocker");
}

#[test]
fn repeated_and_concurrent_attempts_never_overwrite_retained_archives() {
    let temporary = TempDir::new().expect("temporary directory");
    let root = temporary.path().join("releases");
    let server = FixtureServer::start(MAC_ZIP);
    let mut paths = Vec::new();
    thread::scope(|scope| {
        let attempts: Vec<_> = (0..3)
            .map(|index| {
                let root = root.clone();
                let server = &server;
                let installed = temporary.path().join(format!("installed-{index}"));
                fs::write(&installed, b"old binary").expect("old executable");
                scope.spawn(move || {
                    let retention = ArchiveRetention::in_directory(root);
                    let updater = updater(server, MAC_NAME, "aarch64-apple-darwin", &installed, "1.0.0", retention.clone());
                    retention.complete(updater.update()).expect("concurrent update");
                    retention.saved_path().expect("state").expect("saved archive")
                })
            })
            .collect();
        for attempt in attempts {
            paths.push(attempt.join().expect("completed concurrent attempt"));
        }
    });
    let first = paths[0].clone();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), 3);
    for path in paths {
        assert_eq!(fs::read(path).expect("each archive survives"), MAC_ZIP);
    }
    let retention = ArchiveRetention::in_directory(root);
    let downloaded = temporary.path().join(MAC_NAME);
    fs::write(&downloaded, MAC_ZIP).expect("repeat download");
    retention.verify(&downloaded).expect("repeat retention");
    assert_ne!(retention.saved_path().expect("state").expect("new archive"), first);
    assert_eq!(fs::read(first).expect("first archive unchanged"), MAC_ZIP);
}

#[test]
fn checking_or_finding_no_update_creates_no_retention_directory() {
    let temporary = TempDir::new().expect("temporary directory");
    let root = temporary.path().join("releases");
    let retention = ArchiveRetention::in_directory(root.clone());
    let server = FixtureServer::start(MAC_ZIP);
    let updater = updater(
        &server,
        MAC_NAME,
        "aarch64-apple-darwin",
        &temporary.path().join("installed"),
        "1.1.0",
        retention.clone(),
    );
    let latest = updater.get_latest_release().expect("check release without downloading");
    assert!(!latest.is_update_available().expect("version comparison"));
    retention.complete(updater.update()).expect("already up to date");
    assert!(!root.exists());
    assert!(retention.saved_path().expect("state").is_none());
    assert_eq!(server.requests.load(Ordering::SeqCst), 0);
}

#[test]
fn unsafe_original_asset_name_is_rejected_before_download_or_retention() {
    let temporary = TempDir::new().expect("temporary directory");
    let root = temporary.path().join("releases");
    let installed = temporary.path().join("installed");
    fs::write(&installed, b"old binary").expect("old executable");
    let retention = ArchiveRetention::in_directory(root.clone());
    let server = FixtureServer::start(MAC_ZIP);
    let updater = updater(
        &server,
        "../oatty-v1.1.0-aarch64-apple-darwin.zip",
        "aarch64-apple-darwin",
        &installed,
        "1.0.0",
        retention.clone(),
    );
    retention
        .complete(updater.update())
        .expect_err("unsafe original name must be rejected");
    assert!(!root.exists());
    assert_eq!(server.requests.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read(installed).expect("unchanged executable"), b"old binary");
}

#[test]
fn missing_platform_data_directory_has_no_current_directory_fallback() {
    assert!(
        release_directory(None)
            .expect_err("missing platform path")
            .to_string()
            .contains("unavailable")
    );
}

#[test]
fn unsafe_or_unsupported_asset_names_are_rejected() {
    for unsafe_name in [
        ".",
        "..",
        "../oatty-v1.1.0-aarch64-apple-darwin.zip",
        "sub\\oatty-v1.1.0-aarch64-apple-darwin.zip",
        "oatty-v1.1.0-aarch64-apple-darwin.zip:stream",
        "oatty-v1.1.0-aarch64-apple-darwin.zip ",
        "oatty-v1.1.0-aarch64-apple-darwin.zip.",
        "CON.zip",
        "NUL.tar.gz",
        "oatty-v1\n-aarch64-apple-darwin.zip",
        "oatty-..-aarch64-apple-darwin.zip",
        "oatty-v1.1.0-x86_64-unknown-linux-gnu.zip",
        "unrelated.zip",
    ] {
        assert!(validate_archive_name(unsafe_name).is_err(), "must reject {unsafe_name:?}");
    }
    assert!(validate_archive_name(MAC_NAME).is_ok());
}
