#[path = "../render_identity.rs"]
mod render_identity;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "cobalt-render-identity-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self { root };
        for dir in [
            "src",
            "vendor",
            "wikidot-forms",
            "relation-impl-derive",
            "misc",
        ] {
            fixture.write_backend(&format!("{dir}/seed.rs"), b"seed");
        }
        for file in [
            "Cargo.toml",
            "Cargo.lock",
            "build.rs",
            "render_identity.rs",
            "config.example.toml",
            "askama.toml",
        ] {
            fixture.write_backend(file, b"seed");
        }
        fixture.write_locale("fluent/en.ftl", b"hello = Hello");
        fixture
    }

    fn backend(&self) -> PathBuf {
        self.root.join("deepwell")
    }

    fn locales(&self) -> PathBuf {
        self.root.join("locales")
    }

    fn write_backend(&self, path: &str, bytes: &[u8]) {
        self.write(&self.backend().join(path), bytes);
    }

    fn write_locale(&self, path: &str, bytes: &[u8]) {
        self.write(&self.locales().join(path), bytes);
    }

    fn write(&self, path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn fingerprint(&self) -> String {
        render_identity::fingerprint(&self.backend(), &self.locales()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn identical_contents_in_different_absolute_roots_match() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.fingerprint().len(), 64);
}

#[test]
fn ignored_changes_do_not_change_fingerprint() {
    let fixture = Fixture::new();
    let before = fixture.fingerprint();
    for path in [
        "src/docs/note.rs",
        "vendor/ftml/tests/case.rs",
        "vendor/ftml/benches/parse.rs",
        "vendor/ftml/examples/example.rs",
        "vendor/ftml/target/generated.rs",
        "vendor/ftml/.git/config",
        "vendor/ftml/README.md",
        "src/notes.md",
        "tests/integration.rs",
        "README.md",
    ] {
        fixture.write_backend(path, b"changed");
    }
    fixture.write_locale("docs/note.ftl", b"changed");
    fixture.write_locale("README.md", b"changed");
    fixture.write(&fixture.root.join("frontend/app.ts"), b"changed");
    assert_eq!(before, fixture.fingerprint());
}

#[cfg(unix)]
#[test]
fn permission_metadata_does_not_change_fingerprint() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let before = fixture.fingerprint();
    let path = fixture.backend().join("src/seed.rs");
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(before, fixture.fingerprint());
}

#[test]
fn runtime_and_dependency_inputs_change_fingerprint() {
    for path in [
        "src/services/render/service.rs",
        "src/services/caddy/caddyfile.j2",
        "vendor/ftml/src/parser.rs",
        "vendor/ftml/conf/blocks.json",
        "wikidot-forms/src/lib.rs",
        "relation-impl-derive/src/lib.rs",
        "misc/config.toml",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "render_identity.rs",
        "config.example.toml",
        "askama.toml",
    ] {
        let fixture = Fixture::new();
        let before = fixture.fingerprint();
        fixture.write_backend(path, b"changed");
        assert_ne!(before, fixture.fingerprint(), "input {path} was ignored");
    }
    let fixture = Fixture::new();
    let before = fixture.fingerprint();
    fixture.write_locale("fluent/en.ftl", b"hello = Goodbye");
    assert_ne!(before, fixture.fingerprint());
}

#[test]
fn additions_and_deletions_change_fingerprint() {
    let fixture = Fixture::new();
    let before = fixture.fingerprint();
    fixture.write_backend("vendor/ftml/conf/modules.toml", b"modules");
    let added = fixture.fingerprint();
    assert_ne!(before, added);
    fs::remove_file(fixture.backend().join("vendor/ftml/conf/modules.toml")).unwrap();
    assert_eq!(before, fixture.fingerprint());
    fs::remove_file(fixture.locales().join("fluent/en.ftl")).unwrap();
    assert_ne!(before, fixture.fingerprint());
}

#[cfg(unix)]
#[test]
fn symlinked_runtime_file_hashes_target_content() {
    let fixture = Fixture::new();
    let link = fixture.backend().join("misc/config.toml");
    std::os::unix::fs::symlink("../config.example.toml", &link).unwrap();
    let before = fixture.fingerprint();
    fixture.write_backend("config.example.toml", b"changed");
    assert_ne!(before, fixture.fingerprint());
}

#[test]
fn absent_required_input_reports_its_path() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.backend().join("Cargo.lock")).unwrap();
    let error =
        render_identity::fingerprint(&fixture.backend(), &fixture.locales()).unwrap_err();
    assert!(error.to_string().contains("Cargo.lock"), "{error}");

    let fixture = Fixture::new();
    fs::remove_dir_all(fixture.backend().join("vendor")).unwrap();
    let error =
        render_identity::fingerprint(&fixture.backend(), &fixture.locales()).unwrap_err();
    assert!(error.to_string().contains("vendor"), "{error}");

    let fixture = Fixture::new();
    fs::remove_dir_all(fixture.locales()).unwrap();
    let error =
        render_identity::fingerprint(&fixture.backend(), &fixture.locales()).unwrap_err();
    assert!(error.to_string().contains("locales"), "{error}");
}

#[test]
fn input_roots_cover_required_files_and_directories() {
    let fixture = Fixture::new();
    let roots = render_identity::input_roots(&fixture.backend(), &fixture.locales());
    for path in [
        fixture.backend().join("src"),
        fixture.backend().join("vendor"),
        fixture.backend().join("Cargo.lock"),
        fixture.backend().join("render_identity.rs"),
        fixture.locales(),
    ] {
        assert!(roots.contains(&path), "missing {}", path.display());
    }
    assert_eq!(roots.len(), 12);
}
