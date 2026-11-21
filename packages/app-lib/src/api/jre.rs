//! Authentication flow interface
use crate::event::emit::{emit_loading, init_loading};
use crate::install::{
	InstallErrorContext, InstallJavaStep, InstallPhaseDetails, InstallPhaseId,
	InstallProgress, InstallProgressReporter,
};
use crate::state::JavaVersion;
use crate::util::fetch::{
	FetchProgressFn, fetch_advanced, fetch_advanced_with_progress,
};
use bytes::Bytes;
use dashmap::DashMap;
use reqwest::Method;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use sysinfo::{MemoryRefreshKind, RefreshKind};

use crate::util::io;
use crate::util::jre::extract_java_version;
use crate::{
	ErrorKind, LoadingBarType, State,
	util::jre::{self},
};
use anyhow::{Context, bail};

mod managed;
pub(crate) use managed::{
	cleanup_old_managed_java, refresh_managed_java,
	refresh_managed_java_at_startup,
};

pub async fn get_java_versions() -> crate::Result<DashMap<u32, JavaVersion>> {
	let state = State::get().await?;

	JavaVersion::get_all(&state.pool).await
}

pub async fn set_java_version(java_version: JavaVersion) -> crate::Result<()> {
	let state = State::get().await?;
	let _runtime_lease = state.content_store.runtime_cache_lock.read().await;
	java_version.upsert(&state.pool).await?;
	Ok(())
}

// Searches for jres on the system given a java version (ex: 1.8, 1.17, 1.18)
// Allow higher allows for versions higher than the given version to be returned ('at least')
pub async fn find_filtered_jres(
	java_version: Option<u32>,
) -> crate::Result<Vec<JavaVersion>> {
	let state = State::get().await?;
	let jres = jre::get_all_jre(&state.directories.java_versions_dir()).await?;

	// Filter out JREs that are not 1.17 or higher
	Ok(if let Some(java_version) = java_version {
		jres.into_iter()
			.filter(|jre| {
				let jre_version = extract_java_version(&jre.version);
				if let Ok(jre_version) = jre_version {
					jre_version == java_version
				} else {
					false
				}
			})
			.collect()
	} else {
		jres
	})
}

pub async fn auto_install_java(java_version: u32) -> crate::Result<PathBuf> {
	auto_install_java_with_loading(java_version, true).await
}

pub async fn auto_install_java_with_loading(
	java_version: u32,
	show_loading: bool,
) -> crate::Result<PathBuf> {
	auto_install_java_inner(java_version, show_loading, None).await
}

pub async fn auto_install_java_with_reporter(
	java_version: u32,
	reporter: InstallProgressReporter,
) -> crate::Result<PathBuf> {
	auto_install_java_inner(java_version, false, Some(reporter)).await
}

const JAVA_INSTALL_STEPS: u64 = 4;
const JAVA_DOWNLOAD_PROGRESS_MIN_BYTES: u64 = 256 * 1024;

async fn update_java_install_progress(
	reporter: Option<&InstallProgressReporter>,
	java_version: u32,
	step: InstallJavaStep,
	progress: Option<InstallProgress>,
) -> crate::Result<()> {
	if let Some(reporter) = reporter {
		reporter
			.update(
				InstallPhaseId::PreparingJava,
				progress,
				InstallPhaseDetails::Java {
					major_version: java_version,
					step,
				},
			)
			.await?;
	}

	Ok(())
}

fn java_step_progress(current: u64) -> InstallProgress {
	InstallProgress {
		current,
		total: JAVA_INSTALL_STEPS,
		secondary: None,
	}
}

/// Older Java runs on x64 under emulation, as Mojang ships no Windows arm64
/// runtime for it.
const WINDOWS_ARM64_MIN_NATIVE_JAVA: u32 = 17;
/// Azul publishes a Windows arm64 JDK for Java 17, but no JRE.
const WINDOWS_ARM64_JDK_ONLY_JAVA: u32 = 17;

#[derive(Debug, PartialEq, Eq)]
struct JavaPackageQuery {
	os: &'static str,
	arch: &'static str,
	archive_type: &'static str,
	package_type: &'static str,
}

impl JavaPackageQuery {
	fn new(os: &'static str, arch: &'static str, java_version: u32) -> Self {
		let arch = match arch {
			"x86_64" => "x64",
			"x86" => "i686",
			other => other,
		};
		match (os, arch) {
			("linux", "aarch64") => Self {
				os: "linux-glibc",
				arch,
				archive_type: "tar.gz",
				package_type: "jre",
			},
			("linux", "x64") => Self {
				os,
				arch,
				archive_type: "tar.gz",
				package_type: "jre",
			},
			("windows", "aarch64")
				if java_version < WINDOWS_ARM64_MIN_NATIVE_JAVA =>
			{
				Self {
					os,
					arch: "x64",
					archive_type: "zip",
					package_type: "jre",
				}
			}
			("windows", "aarch64")
				if java_version == WINDOWS_ARM64_JDK_ONLY_JAVA =>
			{
				Self {
					os,
					arch,
					archive_type: "zip",
					package_type: "jdk",
				}
			}
			_ => Self {
				os,
				arch,
				archive_type: "zip",
				package_type: "jre",
			},
		}
	}
}

static JAVA_INSTALL_LOCK: tokio::sync::Mutex<()> =
	tokio::sync::Mutex::const_new(());

#[cfg(any(not(target_os = "macos"), test))]
fn java_executable(install_dir: &Path) -> PathBuf {
	#[cfg(target_os = "macos")]
	{
		install_dir
			.join("Contents")
			.join("Home")
			.join("bin")
			.join("java")
	}

	#[cfg(not(target_os = "macos"))]
	{
		install_dir.join("bin").join(jre::JAVA_BIN)
	}
}

/// Resolves Java within an extracted macOS package.
///
/// Most Azul packages place `Contents/Home/bin/java` directly inside the package directory,
/// while Java 16 and 18 nest it inside a bundle such as `zulu-16.jre/Contents/Home/bin/java`.
/// Prefers the direct layout, otherwise requires exactly one matching `.jre` bundle.
#[cfg(any(target_os = "macos", test))]
fn macos_java_executable(install_dir: &Path) -> crate::Result<PathBuf> {
	let direct = install_dir.join("Contents/Home/bin/java");
	if direct.is_file() {
		return Ok(direct);
	}

	let entries = std::fs::read_dir(install_dir)
		.map_err(|err| io::IOError::with_path(err, install_dir))?;
	let mut candidate = None;
	for entry in entries {
		let entry =
			entry.map_err(|err| io::IOError::with_path(err, install_dir))?;
		let bundle = entry.path();
		if bundle
			.extension()
			.is_some_and(|extension| extension == "jre")
			&& bundle.is_dir()
		{
			let java = bundle.join("Contents/Home/bin/java");
			if java.is_file() {
				if candidate.is_some() {
					bail!(ErrorKind::InputError(
						"Java archive contains multiple Java runtime bundles"
							.to_string()
					));
				}
				candidate = Some(java);
			}
		}
	}
	candidate.context(ErrorKind::InputError(
		"Java archive does not contain the expected executable".to_string(),
	))
}

/// Extracts a Java archive to staging, then moves it into place unless a
/// working copy of the same package is already installed.
#[cfg(all(test, unix))]
async fn install_java_archive(
	file: Bytes,
	archive_type: &'static str,
	directory_name: &str,
	java_versions_dir: &Path,
) -> crate::Result<PathBuf> {
	install_java_archive_checked(
		file,
		archive_type,
		directory_name,
		java_versions_dir,
		None,
	)
	.await
}

async fn install_java_archive_checked(
	file: Bytes,
	archive_type: &'static str,
	directory_name: &str,
	java_versions_dir: &Path,
	package: Option<&managed::Package>,
) -> crate::Result<PathBuf> {
	if let Some(package) = package {
		package.verify_archive(&file)?;
		if package.directory_name(archive_type)? != directory_name {
			bail!("Java package directory does not match the verified archive");
		}
	}
	if !managed::safe_component(directory_name) {
		bail!("invalid Java package directory");
	}
	io::create_dir_all(java_versions_dir).await?;
	managed::validate_storage_root(java_versions_dir)?;
	let staging = tempfile::tempdir_in(java_versions_dir)
		.map_err(|err| io::IOError::with_path(err, java_versions_dir))?;
	let archive_directory = directory_name.to_owned();
	let staging = tokio::task::spawn_blocking(move || {
		if archive_type == "tar.gz" {
			let decoder = flate2::read::GzDecoder::new(file.as_ref());
			for entry in tar::Archive::new(decoder).entries()? {
				let entry = entry?;
				let path = entry.path()?;
				managed::validate_archive_entry(&path, &archive_directory)?;
				if let Some(target) = entry.link_name()? {
					managed::validate_archive_link(
						&path,
						&target,
						&archive_directory,
						entry.header().entry_type().is_hard_link(),
					)?;
				}
			}
			let decoder = flate2::read::GzDecoder::new(file.as_ref());
			tar::Archive::new(decoder).unpack(staging.path()).context(
				ErrorKind::InputError(
					"failed to extract java tar.gz".to_string(),
				),
			)?;
		} else {
			let mut archive = zip::ZipArchive::new(std::io::Cursor::new(file))
				.context(ErrorKind::InputError(
					"failed to read java zip".to_string(),
				))?;
			for index in 0..archive.len() {
				let mut entry = archive.by_index(index)?;
				let name = entry.name()?.to_string();
				managed::validate_archive_entry(
					Path::new(&name),
					&archive_directory,
				)?;
				if entry
					.unix_mode()
					.is_some_and(|mode| mode & 0o170000 == 0o120000)
				{
					let mut target = String::new();
					std::io::Read::read_to_string(&mut entry, &mut target)?;
					managed::validate_archive_link(
						Path::new(&name),
						Path::new(&target),
						&archive_directory,
						false,
					)?;
				}
			}
			archive
				.extract(staging.path())
				.context(ErrorKind::InputError(
					"failed to extract java zip".to_string(),
				))?;
		}
		crate::Result::Ok(staging)
	})
	.await??;

	let extracted_dir = staging.path().join(directory_name);
	#[cfg(target_os = "macos")]
	let extracted_java = macos_java_executable(&extracted_dir)?;
	#[cfg(not(target_os = "macos"))]
	let extracted_java = java_executable(&extracted_dir);
	if !extracted_java.is_file() {
		bail!(ErrorKind::InputError(
			"Java archive does not contain the expected executable".to_string()
		));
	}
	managed::validate_java_path(&extracted_dir, &extracted_java)?;
	let extracted_version =
		jre::check_java_at_filepath(&extracted_java).await?;
	if let Some(package) = package {
		managed::validate_installed_version(package, &extracted_version)?;
		managed::write_receipt(&extracted_dir, package)?;
	}

	let install_dir = java_versions_dir.join(directory_name);
	let relative_java = extracted_java.strip_prefix(&extracted_dir).context(
		ErrorKind::InputError(
			"Java executable is outside the extracted package".to_string(),
		),
	)?;
	let java_path = install_dir.join(relative_java);
	let _install_guard = JAVA_INSTALL_LOCK.lock().await;
	let installed = if java_path.is_file() {
		managed::validate_java_path(&install_dir, &java_path)?;
		jre::check_java_at_filepath(&java_path).await.ok()
	} else {
		None
	};
	if let Some(installed) = installed {
		if let Some(package) = package {
			managed::validate_installed_version(package, &installed)?;
		}
	} else {
		if install_dir.exists() {
			if package.is_some() {
				bail!(
					"the Java package destination already exists but is not a working managed runtime"
				);
			}
			io::remove_dir_all(&install_dir).await?;
		}
		tokio::fs::rename(&extracted_dir, &install_dir)
			.await
			.map_err(|err| io::IOError::with_path(err, &install_dir))?;
	}

	Ok(java_path)
}

async fn auto_install_java_inner(
	java_version: u32,
	show_loading: bool,
	reporter: Option<InstallProgressReporter>,
) -> crate::Result<PathBuf> {
	let state = State::get().await?;
	let _runtime_lease = state.content_store.runtime_cache_lock.read().await;
	let _update = managed::UPDATE_LOCK.lock().await;
	let result = install_java_for_state(
		&state,
		java_version,
		show_loading,
		reporter,
		None,
	)
	.await;
	if result.is_err() {
		managed::forget_success(&state, java_version);
	}
	result
}

async fn install_java_for_state(
	state: &State,
	java_version: u32,
	show_loading: bool,
	reporter: Option<InstallProgressReporter>,
	package: Option<&managed::Package>,
) -> crate::Result<PathBuf> {
	let loading_bar = if show_loading {
		Some(
			init_loading(
				LoadingBarType::JavaDownload {
					version: java_version,
				},
				100.0,
				"Downloading java version",
			)
			.await?,
		)
	} else {
		None
	};

	if let Some(loading_bar) = &loading_bar {
		emit_loading(loading_bar, 0.0, Some("Fetching java version"))?;
	}
	update_java_install_progress(
		reporter.as_ref(),
		java_version,
		InstallJavaStep::FetchingMetadata,
		Some(java_step_progress(1)),
	)
	.await?;
	let JavaPackageQuery {
		os,
		arch,
		archive_type,
		package_type,
	} = JavaPackageQuery::new(
		std::env::consts::OS,
		std::env::consts::ARCH,
		java_version,
	);
	let query = JavaPackageQuery {
		os,
		arch,
		archive_type,
		package_type,
	};
	let download = match package {
		Some(package) => package.clone(),
		None => managed::latest_package(state, java_version, &query).await?,
	};
	if let Some(loading_bar) = &loading_bar {
		emit_loading(loading_bar, 10.0, Some("Downloading java version"))?;
	}

	{
		if let Some(reporter) = &reporter {
			reporter
				.set_context(
					InstallErrorContext::new("download Java archive")
						.urls(vec![download.download_url.clone()])
						.file_path(download.name.clone())
						.java_version(java_version)
						.os(std::env::consts::OS)
						.arch(std::env::consts::ARCH)
						.build(),
				)
				.await?;
		}
		update_java_install_progress(
			reporter.as_ref(),
			java_version,
			InstallJavaStep::Downloading,
			None,
		)
		.await?;
		let file = if reporter.is_some() {
			let mut last_reported_bytes = 0_u64;
			let download_reporter = reporter.clone();
			let mut progress = move |current: u64,
			                         total: u64|
			      -> Pin<
				Box<dyn Future<Output = crate::Result<()>> + Send>,
			> {
				let min_delta =
					(total / 200).max(JAVA_DOWNLOAD_PROGRESS_MIN_BYTES);
				if current < total
					&& current.saturating_sub(last_reported_bytes) < min_delta
				{
					return Box::pin(async { Ok(()) });
				}

				last_reported_bytes = current;
				let reporter = download_reporter.clone();
				Box::pin(async move {
					update_java_install_progress(
						reporter.as_ref(),
						java_version,
						InstallJavaStep::Downloading,
						Some(InstallProgress {
							current,
							total,
							secondary: None,
						}),
					)
					.await
				})
			};

			fetch_advanced_with_progress(
				Method::GET,
				&download.download_url,
				None,
				None,
				None,
				None,
				loading_bar.as_ref().map(|loading_bar| (loading_bar, 80.0)),
				None,
				&state.fetch_semaphore,
				&state.pool,
				Some(&mut progress as &mut FetchProgressFn<'_>),
			)
			.await?
		} else {
			fetch_advanced(
				Method::GET,
				&download.download_url,
				None,
				None,
				None,
				None,
				loading_bar.as_ref().map(|loading_bar| (loading_bar, 80.0)),
				None,
				&state.fetch_semaphore,
				&state.pool,
			)
			.await?
		};

		let path = state.directories.java_versions_dir();

		if let Some(reporter) = &reporter {
			reporter
				.set_context(
					InstallErrorContext::new("read Java archive")
						.urls(vec![download.download_url.clone()])
						.file_path(download.name.clone())
						.target_path(path.display().to_string())
						.java_version(java_version)
						.os(std::env::consts::OS)
						.arch(std::env::consts::ARCH)
						.build(),
				)
				.await?;
		}
		let directory_name = download.directory_name(archive_type)?;
		if let Some(loading_bar) = &loading_bar {
			emit_loading(loading_bar, 0.0, Some("Extracting java"))?;
		}
		update_java_install_progress(
			reporter.as_ref(),
			java_version,
			InstallJavaStep::Extracting,
			Some(java_step_progress(3)),
		)
		.await?;
		if let Some(reporter) = &reporter {
			reporter
				.set_context(
					InstallErrorContext::new("extract Java archive")
						.urls(vec![download.download_url.clone()])
						.file_path(download.name.clone())
						.target_path(path.display().to_string())
						.java_version(java_version)
						.os(std::env::consts::OS)
						.arch(std::env::consts::ARCH)
						.build(),
				)
				.await?;
		}
		let java_path = install_java_archive_checked(
			file,
			archive_type,
			directory_name,
			&path,
			Some(&download),
		)
		.await?;
		if let Some(loading_bar) = &loading_bar {
			emit_loading(loading_bar, 10.0, Some("Done extracting java"))?;
		}

		if package.is_none() {
			managed::record_success(state, java_version, &download);
		}
		Ok(java_path)
	}
}

// Validates JRE at a given at a given path
pub async fn check_jre(path: PathBuf) -> crate::Result<JavaVersion> {
	jre::check_java_at_filepath(&path).await
}

// Test JRE at a given path
pub async fn test_jre(
	path: PathBuf,
	major_version: u32,
) -> crate::Result<bool> {
	let jre = match jre::check_java_at_filepath(&path).await {
		Ok(jre) => jre,
		Err(e) => {
			tracing::warn!("Invalid Java at {}: {e}", path.display());
			return Ok(false);
		}
	};
	let version = extract_java_version(&jre.version)?;
	tracing::info!(
		"Expected Java version {major_version}, and found {version} at {}",
		path.display()
	);
	Ok(version == major_version)
}

fn system_memory_bytes() -> u64 {
	sysinfo::System::new_with_specifics(
		RefreshKind::nothing()
			.with_memory(MemoryRefreshKind::nothing().with_ram()),
	)
	.total_memory()
}

/// Recommended default max heap (MiB) for new instances based on system RAM.
pub fn default_memory_max_mb() -> u32 {
	const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
	let system_gib = system_memory_bytes() / BYTES_PER_GIB;

	if system_gib < 8 {
		1024 * 2
	} else if system_gib >= 24 {
		1024 * 6
	} else {
		1024 * 4
	}
}

// Gets maximum memory in KiB.
pub async fn get_max_memory() -> crate::Result<u64> {
	Ok(system_memory_bytes() / 1024)
}

#[cfg(test)]
fn canonical_temporary_directory() -> tempfile::TempDir {
	let parent = dunce::canonicalize(std::env::temp_dir()).unwrap();
	tempfile::tempdir_in(parent).unwrap()
}

#[cfg(test)]
mod tests {
	use super::JavaPackageQuery;
	#[cfg(unix)]
	use super::{install_java_archive, java_executable};
	#[cfg(unix)]
	use bytes::Bytes;
	#[cfg(unix)]
	use std::{io::Write, os::unix::fs::PermissionsExt, path::Path};

	#[cfg(unix)]
	const PACKAGE: &str = "zulu-test";
	#[cfg(unix)]
	const WORKING_JAVA: &[u8] =
		b"#!/bin/sh\necho os.arch=aarch64\necho java.version=21.0.1\n";
	#[cfg(unix)]
	const BROKEN_JAVA: &[u8] = b"#!/bin/sh\nexit 1\n";

	#[cfg(unix)]
	fn java_entry() -> String {
		java_executable(Path::new(PACKAGE))
			.to_string_lossy()
			.into_owned()
	}

	#[cfg(unix)]
	fn tar_gz(path: &str, contents: &[u8]) -> Bytes {
		let encoder = flate2::write::GzEncoder::new(
			Vec::new(),
			flate2::Compression::fast(),
		);
		let mut builder = tar::Builder::new(encoder);
		let mut header = tar::Header::new_gnu();
		header.set_size(contents.len() as u64);
		header.set_mode(0o755);
		header.set_cksum();
		builder.append_data(&mut header, path, contents).unwrap();
		builder.into_inner().unwrap().finish().unwrap().into()
	}

	#[cfg(unix)]
	fn zip_archive(path: &str, contents: &[u8]) -> Bytes {
		let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
		let options = zip::write::SimpleFileOptions::default()
			.compression_method(zip::CompressionMethod::Stored)
			.unix_permissions(0o755);
		writer.start_file(path, options).unwrap();
		writer.write_all(contents).unwrap();
		writer.finish().unwrap().into_inner().into()
	}

	#[cfg(unix)]
	fn write_java(
		java_versions_dir: &Path,
		contents: &[u8],
	) -> std::path::PathBuf {
		let java = java_executable(&java_versions_dir.join(PACKAGE));
		std::fs::create_dir_all(java.parent().unwrap()).unwrap();
		std::fs::write(&java, contents).unwrap();
		std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755))
			.unwrap();
		java
	}

	#[cfg(unix)]
	fn assert_no_staging_left(java_versions_dir: &Path) {
		let entries = std::fs::read_dir(java_versions_dir)
			.unwrap()
			.map(|entry| entry.unwrap().file_name())
			.collect::<Vec<_>>();
		assert_eq!(entries, [PACKAGE]);
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn installs_archives_with_executable_java() {
		for (archive_type, file) in [
			("tar.gz", tar_gz(&java_entry(), WORKING_JAVA)),
			("zip", zip_archive(&java_entry(), WORKING_JAVA)),
		] {
			let dir = super::canonical_temporary_directory();
			let java =
				install_java_archive(file, archive_type, PACKAGE, dir.path())
					.await
					.unwrap();
			assert_eq!(java, java_executable(&dir.path().join(PACKAGE)));
			assert_eq!(std::fs::read(&java).unwrap(), WORKING_JAVA);
			let mode = std::fs::metadata(&java).unwrap().permissions().mode();
			assert_ne!(mode & 0o111, 0);
			assert_no_staging_left(dir.path());
		}
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn rejected_archive_leaves_existing_install_untouched() {
		let dir = super::canonical_temporary_directory();
		let java = write_java(dir.path(), BROKEN_JAVA);
		let file = tar_gz(&format!("{PACKAGE}/readme.txt"), b"no java here");
		install_java_archive(file, "tar.gz", PACKAGE, dir.path())
			.await
			.unwrap_err();
		assert_eq!(std::fs::read(java).unwrap(), BROKEN_JAVA);
		assert_no_staging_left(dir.path());
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn broken_install_is_replaced() {
		let dir = super::canonical_temporary_directory();
		let java = write_java(dir.path(), BROKEN_JAVA);
		let stale = dir.path().join(PACKAGE).join("stale.txt");
		std::fs::write(&stale, "stale").unwrap();
		let file = tar_gz(&java_entry(), WORKING_JAVA);
		install_java_archive(file, "tar.gz", PACKAGE, dir.path())
			.await
			.unwrap();
		assert_eq!(std::fs::read(java).unwrap(), WORKING_JAVA);
		assert!(!stale.exists());
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn working_install_is_kept() {
		let dir = super::canonical_temporary_directory();
		write_java(dir.path(), WORKING_JAVA);
		let marker = dir.path().join(PACKAGE).join("in-use.txt");
		std::fs::write(&marker, "in use").unwrap();
		let file = tar_gz(&java_entry(), WORKING_JAVA);
		install_java_archive(file, "tar.gz", PACKAGE, dir.path())
			.await
			.unwrap();
		assert!(marker.exists());
		assert_no_staging_left(dir.path());
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn parallel_installs_of_one_package_all_succeed() {
		let dir = super::canonical_temporary_directory();
		let file = tar_gz(&java_entry(), WORKING_JAVA);
		let (first, second, third) = tokio::join!(
			install_java_archive(file.clone(), "tar.gz", PACKAGE, dir.path()),
			install_java_archive(file.clone(), "tar.gz", PACKAGE, dir.path()),
			install_java_archive(file, "tar.gz", PACKAGE, dir.path()),
		);
		let java = first.unwrap();
		assert_eq!(second.unwrap(), java);
		assert_eq!(third.unwrap(), java);
		assert_no_staging_left(dir.path());
	}

	#[test]
	fn resolves_macos_java_layouts() {
		for relative in [
			"Contents/Home/bin/java",
			"zulu-16.jre/Contents/Home/bin/java",
			"zulu-18.jre/Contents/Home/bin/java",
		] {
			let dir = super::canonical_temporary_directory();
			let java = dir.path().join(relative);
			std::fs::create_dir_all(java.parent().unwrap()).unwrap();
			std::fs::write(&java, b"java").unwrap();
			assert_eq!(super::macos_java_executable(dir.path()).unwrap(), java);
		}
	}

	#[test]
	fn rejects_missing_or_ambiguous_macos_java() {
		let dir = super::canonical_temporary_directory();
		assert!(super::macos_java_executable(dir.path()).is_err());
		for bundle in ["first.jre", "second.jre"] {
			let java = dir.path().join(bundle).join("Contents/Home/bin/java");
			std::fs::create_dir_all(java.parent().unwrap()).unwrap();
			std::fs::write(java, b"java").unwrap();
		}
		assert!(
			super::macos_java_executable(dir.path())
				.unwrap_err()
				.to_string()
				.contains("multiple")
		);
	}

	#[test]
	fn linux_arm64_uses_glibc_tarballs() {
		assert_eq!(
			JavaPackageQuery::new("linux", "aarch64", 16),
			JavaPackageQuery {
				os: "linux-glibc",
				arch: "aarch64",
				archive_type: "tar.gz",
				package_type: "jre",
			}
		);
	}

	#[test]
	fn linux_x64_uses_tarballs() {
		for java_version in [8, 16, 17, 21, 25] {
			assert_eq!(
				JavaPackageQuery::new("linux", "x86_64", java_version),
				JavaPackageQuery {
					os: "linux",
					arch: "x64",
					archive_type: "tar.gz",
					package_type: "jre",
				}
			);
		}
	}

	#[test]
	fn windows_arm64_runs_older_java_under_emulation() {
		for java_version in [8, 16] {
			let query =
				JavaPackageQuery::new("windows", "aarch64", java_version);
			assert_eq!(query.arch, "x64");
			assert_eq!(query.package_type, "jre");
		}
	}

	#[test]
	fn windows_arm64_uses_native_java_from_17() {
		let query = JavaPackageQuery::new("windows", "aarch64", 17);
		assert_eq!(query.arch, "aarch64");
		assert_eq!(query.package_type, "jdk");
		for java_version in [21, 25] {
			let query =
				JavaPackageQuery::new("windows", "aarch64", java_version);
			assert_eq!(query.arch, "aarch64");
			assert_eq!(query.package_type, "jre");
		}
	}

	#[test]
	fn other_platforms_use_native_zips() {
		for (os, arch) in [("windows", "x86_64"), ("macos", "aarch64")] {
			assert_eq!(
				JavaPackageQuery::new(os, arch, 8),
				JavaPackageQuery {
					os,
					arch: if arch == "x86_64" { "x64" } else { arch },
					archive_type: "zip",
					package_type: "jre",
				}
			);
		}
	}
}

#[cfg(test)]
mod archive_safety_tests {
	use super::*;
	use std::io::Write;

	fn archive(path: &str, contents: &[u8]) -> Bytes {
		let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
		writer
			.start_file(
				path,
				zip::write::SimpleFileOptions::default()
					.unix_permissions(0o755),
			)
			.unwrap();
		writer.write_all(contents).unwrap();
		writer.finish().unwrap().into_inner().into()
	}

	fn existing_java(root: &Path, directory: &str) -> PathBuf {
		let path = java_executable(&root.join(directory));
		std::fs::create_dir_all(path.parent().unwrap()).unwrap();
		std::fs::write(&path, b"existing runtime remains available").unwrap();
		path
	}

	#[tokio::test]
	async fn untrusted_archive_paths_leave_existing_runtime_untouched() {
		for path in [
			"other/bin/java",
			"zulu-test/../outside",
			"zulu-test/.bytelauncher-java.json",
		] {
			let storage = super::canonical_temporary_directory();
			let old = existing_java(storage.path(), "zulu-test");
			let file = archive(path, b"untrusted");
			assert!(
				install_java_archive_checked(
					file,
					"zip",
					"zulu-test",
					storage.path(),
					None
				)
				.await
				.is_err()
			);
			assert_eq!(
				std::fs::read(&old).unwrap(),
				b"existing runtime remains available"
			);
			assert_eq!(std::fs::read_dir(storage.path()).unwrap().count(), 1);
		}
	}

	#[tokio::test]
	async fn failed_staged_runtime_validation_retains_previous_installation() {
		let storage = super::canonical_temporary_directory();
		let old = existing_java(storage.path(), "zulu-test");
		let relative = java_executable(Path::new("zulu-test"))
			.to_string_lossy()
			.replace('\\', "/");
		let file =
			archive(&relative, b"this is not an executable Java runtime");
		assert!(
			install_java_archive_checked(
				file,
				"zip",
				"zulu-test",
				storage.path(),
				None
			)
			.await
			.is_err()
		);
		assert_eq!(
			std::fs::read(&old).unwrap(),
			b"existing runtime remains available"
		);
		assert_eq!(std::fs::read_dir(storage.path()).unwrap().count(), 1);
	}

	#[tokio::test]
	async fn installer_checks_vendor_digest_before_extracting_or_publishing() {
		let storage = super::canonical_temporary_directory();
		let directory = "zulu21.54.201-ca-jre21.0.13.1-win_x64";
		let old = existing_java(storage.path(), directory);
		let package: managed::Package =
			serde_json::from_value(serde_json::json!({
				"name": format!("{directory}.zip"),
				"download_url": format!("https://cdn.azul.com/zulu/bin/{directory}.zip"),
				"package_uuid": "b74183d9-e32f-4ba4-a923-9420a32566bd",
				"java_version": [21,0,13,1],
				"sha256_hash": "0".repeat(64),
			}))
			.unwrap();
		let error = install_java_archive_checked(
			Bytes::from_static(b"corrupted archive"),
			"zip",
			directory,
			storage.path(),
			Some(&package),
		)
		.await
		.unwrap_err();
		assert!(error.to_string().contains("SHA-256"));
		assert_eq!(
			std::fs::read(&old).unwrap(),
			b"existing runtime remains available"
		);
		assert_eq!(std::fs::read_dir(storage.path()).unwrap().count(), 1);
	}
}
