use super::*;
use std::fs;

const DISTRIBUTION: &str = "zulu21.48.17-ca-jre21.0.10-win_x64";
const ABC_SHA256: &str =
	"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn package(name: &str, version: &[u32]) -> Package {
	serde_json::from_value(serde_json::json!({
		"download_url": format!("https://cdn.azul.com/zulu/bin/{name}.zip"),
		"name": format!("{name}.zip"),
		"package_uuid": "550e8400-e29b-41d4-a716-446655440000",
		"java_version": version,
		"distro_version": [21, 48, 17],
		"openjdk_build_number": 7,
		"sha256_hash": ABC_SHA256,
		"arch": "x86",
		"hw_bitness": 64
	}))
	.unwrap()
}

fn fixture(root: &Path, name: &str, vendor: &str, version: &str) -> PathBuf {
	let directory = root.join(name);
	let executable = super::super::java_executable(&directory);
	fs::create_dir_all(executable.parent().unwrap()).unwrap();
	fs::write(&executable, b"Java fixture; never executed").unwrap();
	let home = executable.parent().unwrap().parent().unwrap();
	fs::write(
		home.join("release"),
		format!(
			"IMPLEMENTOR=\"{vendor}\"\nJAVA_VERSION=\"{version}\"\nJAVA_RUNTIME_VERSION=\"{version}+7-LTS\"\n"
		),
	)
	.unwrap();
	directory
}

fn owned_fixture(root: &Path) -> PathBuf {
	fixture(root, DISTRIBUTION, "Azul Systems, Inc.", "21.0.10")
}

fn assert_not_owned(root: &Path, directory: &Path) {
	assert!(
		!matches!(owned_distribution(root, directory), Ok(Some(_))),
		"untrusted directory was admitted for managed replacement or cleanup: {}",
		directory.display()
	);
	assert!(directory.exists());
}

#[test]
fn legacy_distribution_requires_matching_release_and_vendor() {
	let temporary = tempfile::tempdir().unwrap();
	let root = dunce::canonicalize(temporary.path()).unwrap();
	let directory = owned_fixture(&root);
	let owned = owned_distribution(&root, &directory).unwrap().unwrap();
	assert_eq!(owned.directory, directory);
	assert_eq!(owned.executable, super::super::java_executable(&directory));
	assert_eq!(owned.release, JavaRelease::parse("21.0.10+7").unwrap());
	assert!(owned_selection(&root, &owned.executable).unwrap().is_some());
	assert!(!directory.join(RECEIPT).exists());
}

#[test]
fn misleading_legacy_names_and_release_files_are_retained() {
	for (name, vendor, version) in [
		("custom-java", "Azul Systems, Inc.", "21.0.10"),
		("zulu-user-java", "Azul Systems, Inc.", "21.0.10"),
		(DISTRIBUTION, "Eclipse Adoptium", "21.0.10"),
		(DISTRIBUTION, "Azul Systems, Inc.", "21.0.9"),
		(DISTRIBUTION, "Azul Systems, Inc.", "17.0.10"),
		(DISTRIBUTION, "Azul Systems, Inc.", "21-ea"),
	] {
		let temporary = tempfile::tempdir().unwrap();
		let root = dunce::canonicalize(temporary.path()).unwrap();
		let directory = fixture(&root, name, vendor, version);
		assert_not_owned(&root, &directory);
		assert!(installed_distributions(&root).unwrap().is_empty());
		assert!(super::super::java_executable(&directory).is_file());
	}
}

#[test]
fn unknown_siblings_and_nested_runtimes_are_not_cleanup_candidates() {
	let temporary = tempfile::tempdir().unwrap();
	let root = dunce::canonicalize(temporary.path()).unwrap();
	let known = owned_fixture(&root);
	let unknown = fixture(&root, "my-java", "Azul Systems, Inc.", "21.0.10");
	let nested = owned_fixture(&root.join("user-provided"));
	let sibling = root.join("README-user-data.txt");
	fs::write(&sibling, b"preserve this user file").unwrap();
	let distributions = installed_distributions(&root).unwrap();
	assert_eq!(distributions.len(), 1);
	assert_eq!(distributions[0].directory, known);
	assert_not_owned(&root, &nested);
	assert!(
		owned_selection(&root, &super::super::java_executable(&nested))
			.unwrap()
			.is_none()
	);
	assert!(unknown.is_dir());
	assert_eq!(fs::read(sibling).unwrap(), b"preserve this user file");
}

#[test]
fn valid_looking_external_distribution_is_never_managed() {
	let temporary = tempfile::tempdir().unwrap();
	let root = dunce::canonicalize(temporary.path()).unwrap();
	let external = tempfile::tempdir().unwrap();
	let external_root = dunce::canonicalize(external.path()).unwrap();
	let directory = owned_fixture(&external_root);
	let executable = super::super::java_executable(&directory);
	assert_not_owned(&root, &directory);
	assert!(owned_selection(&root, &executable).unwrap().is_none());
	assert!(executable.is_file());
}

#[test]
fn valid_receipt_is_correlated_with_the_legacy_distribution() {
	let temporary = tempfile::tempdir().unwrap();
	let root = dunce::canonicalize(temporary.path()).unwrap();
	let directory = owned_fixture(&root);
	let package = package(DISTRIBUTION, &[21, 0, 10]);
	write_receipt(&directory, &package).unwrap();
	assert!(owned_distribution(&root, &directory).unwrap().is_some());
	let original = fs::read(directory.join(RECEIPT)).unwrap();
	assert!(write_receipt(&directory, &package).is_err());
	assert_eq!(fs::read(directory.join(RECEIPT)).unwrap(), original);
}

#[test]
fn inconsistent_or_malicious_receipts_are_retained_without_adoption() {
	let valid = serde_json::json!({
		"format": 1,
		"provider": "azul-zulu",
		"package": package(DISTRIBUTION, &[21, 0, 10]),
	});
	let mut cases = Vec::new();
	for (field, replacement) in [
		("provider", serde_json::json!("user-provided")),
		("format", serde_json::json!(2)),
	] {
		let mut receipt = valid.clone();
		receipt[field] = replacement;
		cases.push(serde_json::to_vec(&receipt).unwrap());
	}
	for (field, replacement) in [
		("name", serde_json::json!("../../outside.zip")),
		(
			"download_url",
			serde_json::json!("https://example.com/runtime.zip"),
		),
		("package_uuid", serde_json::json!("invalid-uuid")),
		("sha256_hash", serde_json::json!(null)),
		("java_version", serde_json::json!([21, 0, 11])),
	] {
		let mut receipt = valid.clone();
		receipt["package"][field] = replacement;
		cases.push(serde_json::to_vec(&receipt).unwrap());
	}
	cases.push(b"not a receipt".to_vec());
	for bytes in cases {
		let temporary = tempfile::tempdir().unwrap();
		let root = dunce::canonicalize(temporary.path()).unwrap();
		let directory = owned_fixture(&root);
		fs::write(directory.join(RECEIPT), &bytes).unwrap();
		assert_not_owned(&root, &directory);
		assert!(installed_distributions(&root).unwrap().is_empty());
		assert_eq!(fs::read(directory.join(RECEIPT)).unwrap(), bytes);
	}
}

#[test]
fn checksum_validation_rejects_corruption_and_missing_vendor_proof() {
	let mut package = package(DISTRIBUTION, &[21, 0, 10]);
	assert!(package.verify_archive(b"abc").is_ok());
	assert!(package.verify_archive(b"abd").is_err());
	package.sha256_hash = Some(ABC_SHA256.to_ascii_uppercase());
	assert!(package.verify_archive(b"abc").is_ok());
	for checksum in [None, Some("".to_owned()), Some("g".repeat(64))] {
		package.sha256_hash = checksum;
		assert!(package.verify_archive(b"abc").is_err());
	}
}

#[test]
fn validated_runtime_must_match_all_security_version_components() {
	let package = package(DISTRIBUTION, &[21, 0, 10]);
	let mut installed = JavaVersion {
		parsed_version: 21,
		version: "21.0.10+7-LTS".to_owned(),
		architecture: "x86_64".to_owned(),
		path: "unused-java-fixture".to_owned(),
	};
	assert!(validate_installed_version(&package, &installed).is_ok());
	for version in ["21.0.9+7", "21.0.10.0.1+7", "17.0.10+7", "21-ea"] {
		installed.version = version.to_owned();
		assert!(validate_installed_version(&package, &installed).is_err());
	}
	installed.version = "21.0.10+7".to_owned();
	installed.parsed_version = 17;
	assert!(validate_installed_version(&package, &installed).is_err());
}

#[test]
fn fifth_security_component_outranks_vendor_and_build_numbers() {
	let regular = package(DISTRIBUTION, &[21, 0, 13]);
	let mut security = regular.clone();
	security.java_version = vec![21, 0, 13, 0, 1];
	security.openjdk_build_number = 1;
	let mut regular = regular;
	regular.openjdk_build_number = 999;
	regular.distro_version = vec![999, 999, 999];
	assert!(security.release() > regular.release());
	assert!(
		JavaRelease::parse("21.0.13.0.1+1").unwrap()
			> JavaRelease::parse("21.0.13+999").unwrap()
	);
	assert_eq!(
		JavaRelease::parse("21.0.13.0.0+1"),
		JavaRelease::parse("21.0.13+1")
	);
}

#[test]
fn replacement_only_migrates_owned_same_major_without_downgrading() {
	let temporary = tempfile::tempdir().unwrap();
	let root = dunce::canonicalize(temporary.path()).unwrap();
	let old = owned_fixture(&root);
	let replacement_directory = fixture(
		&root,
		"zulu21.54.201-ca-jre21.0.13.0.1-win_x64",
		"Azul Systems, Inc.",
		"21.0.13.0.1",
	);
	let replacement = owned_distribution(&root, &replacement_directory)
		.unwrap()
		.unwrap();
	assert_eq!(
		replacement_for_selection(
			&root,
			&super::super::java_executable(&old),
			&replacement
		)
		.unwrap(),
		Some(replacement.executable.clone())
	);
	let newer = fixture(
		&root,
		"zulu21.54.201-ca-jre21.0.14-win_x64",
		"Azul Systems, Inc.",
		"21.0.14",
	);
	let other_major = fixture(
		&root,
		"zulu17.54.21-ca-jre17.0.13-win_x64",
		"Azul Systems, Inc.",
		"17.0.13",
	);
	let external = tempfile::tempdir().unwrap();
	let external_root = dunce::canonicalize(external.path()).unwrap();
	let external_directory = owned_fixture(&external_root);
	for directory in [&newer, &other_major, &external_directory] {
		let executable = super::super::java_executable(directory);
		assert!(
			replacement_for_selection(&root, &executable, &replacement)
				.unwrap()
				.is_none()
		);
		assert!(executable.is_file());
	}
}

#[cfg(windows)]
#[test]
fn managed_windows_console_java_keeps_its_executable_choice() {
	let temporary = tempfile::tempdir().unwrap();
	let root = dunce::canonicalize(temporary.path()).unwrap();
	let old = owned_fixture(&root);
	let old_console =
		super::super::java_executable(&old).with_file_name("java.exe");
	fs::write(&old_console, b"console Java fixture; never executed").unwrap();
	let replacement_directory = fixture(
		&root,
		"zulu21.54.201-ca-jre21.0.13.1-win_x64",
		"Azul Systems, Inc.",
		"21.0.13.1",
	);
	let replacement = owned_distribution(&root, &replacement_directory)
		.unwrap()
		.unwrap();
	let new_console = replacement.executable.with_file_name("java.exe");
	fs::write(&new_console, b"replacement console fixture").unwrap();
	assert_eq!(
		replacement_for_selection(&root, &old_console, &replacement).unwrap(),
		Some(new_console)
	);
	assert_eq!(
		replacement_for_selection(
			&root,
			&super::super::java_executable(&old),
			&replacement
		)
		.unwrap(),
		Some(replacement.executable)
	);
}

#[cfg(any(unix, windows))]
fn symlink(source: &Path, destination: &Path, directory: bool) -> bool {
	#[cfg(unix)]
	let result = {
		let _ = directory;
		std::os::unix::fs::symlink(source, destination)
	};
	#[cfg(windows)]
	let result = if directory {
		std::os::windows::fs::symlink_dir(source, destination)
	} else {
		std::os::windows::fs::symlink_file(source, destination)
	};
	match result {
		Ok(()) => true,
		#[cfg(windows)]
		Err(error) if error.raw_os_error() == Some(1314) => {
			eprintln!(
				"Skipping symlink fixture: Windows symlink privilege is unavailable"
			);
			false
		}
		Err(error) => panic!("could not create symlink fixture: {error}"),
	}
}

#[cfg(any(unix, windows))]
#[test]
fn symlinked_storage_or_distribution_roots_are_never_adopted() {
	let temporary = tempfile::tempdir().unwrap();
	let parent = dunce::canonicalize(temporary.path()).unwrap();
	let root = parent.join("managed");
	let external = parent.join("external");
	fs::create_dir_all(&root).unwrap();
	let directory = owned_fixture(&external);
	let aliased_root = parent.join("aliased-root");
	if !symlink(&external, &aliased_root, true) {
		return;
	}
	assert!(validate_storage_root(&aliased_root).is_err());
	assert!(installed_distributions(&aliased_root).is_err());
	let aliased_package = root.join(DISTRIBUTION);
	assert!(symlink(&directory, &aliased_package, true));
	assert_not_owned(&root, &aliased_package);
	assert!(installed_distributions(&root).unwrap().is_empty());
	assert!(
		owned_selection(
			&root,
			&super::super::java_executable(&aliased_package)
		)
		.unwrap()
		.is_none()
	);
	assert!(super::super::java_executable(&directory).is_file());
}

#[cfg(any(unix, windows))]
#[test]
fn java_executable_link_cannot_escape_the_package_directory() {
	let temporary = tempfile::tempdir().unwrap();
	let parent = dunce::canonicalize(temporary.path()).unwrap();
	let root = parent.join("managed");
	let directory = owned_fixture(&root);
	let executable = super::super::java_executable(&directory);
	let external = parent.join("external-java");
	fs::write(&external, b"external Java must survive").unwrap();
	fs::remove_file(&executable).unwrap();
	if !symlink(&external, &executable, false) {
		return;
	}
	assert!(validate_java_path(&directory, &executable).is_err());
	assert_not_owned(&root, &directory);
	assert!(installed_distributions(&root).unwrap().is_empty());
	assert_eq!(fs::read(external).unwrap(), b"external Java must survive");
}

#[cfg(any(unix, windows))]
#[test]
fn receipt_and_release_links_do_not_grant_ownership_or_overwrite_targets() {
	for filename in [RECEIPT, "release"] {
		let temporary = tempfile::tempdir().unwrap();
		let parent = dunce::canonicalize(temporary.path()).unwrap();
		let root = parent.join("managed");
		let directory = owned_fixture(&root);
		let executable = super::super::java_executable(&directory);
		let target = parent.join("external-file");
		fs::write(&target, b"external file must survive").unwrap();
		let link = if filename == RECEIPT {
			directory.join(RECEIPT)
		} else {
			let release = executable
				.parent()
				.unwrap()
				.parent()
				.unwrap()
				.join("release");
			fs::remove_file(&release).unwrap();
			release
		};
		if !symlink(&target, &link, false) {
			return;
		}
		assert_not_owned(&root, &directory);
		assert!(installed_distributions(&root).unwrap().is_empty());
		if filename == RECEIPT {
			assert!(
				write_receipt(&directory, &package(DISTRIBUTION, &[21, 0, 10]))
					.is_err()
			);
		}
		assert_eq!(fs::read(target).unwrap(), b"external file must survive");
	}
}
