use super::JavaPackageQuery;
use crate::State;
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{JavaVersion, runtime_cache};
use crate::util::fetch::fetch_json;
use anyhow::{Context, bail};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub(super) static UPDATE_LOCK: Mutex<()> = Mutex::const_new(());
static CHECKS: LazyLock<
	std::sync::Mutex<HashMap<(PathBuf, u32), CheckedPackage>>,
> = LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
const CHECK_INTERVAL: Duration = Duration::from_secs(3600);
const RECEIPT: &str = ".bytelauncher-java.json";

struct CheckedPackage {
	checked_at: Instant,
	package: Package,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Package {
	pub download_url: String,
	pub name: String,
	pub package_uuid: String,
	pub java_version: Vec<u32>,
	#[serde(default)]
	pub distro_version: Vec<u32>,
	#[serde(default)]
	pub openjdk_build_number: u32,
	#[serde(default)]
	pub sha256_hash: Option<String>,
	#[serde(default)]
	arch: String,
	#[serde(default)]
	hw_bitness: u32,
	#[serde(default)]
	crs_supported: bool,
	#[serde(default)]
	crac_supported: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct JavaRelease {
	version: Vec<u32>,
	build: u32,
}

impl JavaRelease {
	fn parse(raw: &str) -> Option<Self> {
		let (version, build) = raw
			.split_once('+')
			.or_else(|| raw.split_once("-b"))
			.map_or((raw, 0), |(version, build)| {
				(
					version,
					build
						.split('-')
						.next()
						.and_then(|x| x.parse().ok())
						.unwrap_or(0),
				)
			});
		let version = version.strip_suffix("-LTS").unwrap_or(version);
		let legacy = version.starts_with("1.");
		let normalized = version.replace('_', ".");
		let mut components = normalized
			.split('.')
			.map(str::parse::<u32>)
			.collect::<Result<Vec<_>, _>>()
			.ok()?;
		if legacy {
			components.remove(0);
		}
		if components.is_empty() || components[0] == 0 {
			return None;
		}
		while components.len() > 1 && components.last() == Some(&0) {
			components.pop();
		}
		Some(Self {
			version: components,
			build,
		})
	}
}

impl Package {
	fn release(&self) -> Option<JavaRelease> {
		if self.java_version.first().is_none_or(|major| *major == 0) {
			return None;
		}
		let mut version = self.java_version.clone();
		while version.len() > 1 && version.last() == Some(&0) {
			version.pop();
		}
		Some(JavaRelease {
			version,
			build: self.openjdk_build_number,
		})
	}

	pub(super) fn directory_name(
		&self,
		archive_type: &str,
	) -> crate::Result<&str> {
		let directory = self
			.name
			.strip_suffix(&format!(".{archive_type}"))
			.filter(|name| {
				safe_component(name) && legacy_name_version(name).is_some()
			})
			.context("invalid Azul Java archive name")?;
		let url = url::Url::parse(&self.download_url)?;
		if url.scheme() != "https"
			|| url.host_str() != Some("cdn.azul.com")
			|| url
				.path_segments()
				.and_then(|mut segments| segments.next_back())
				!= Some(self.name.as_str())
			|| uuid::Uuid::parse_str(&self.package_uuid).is_err()
		{
			bail!("invalid Azul Java package metadata");
		}
		Ok(directory)
	}

	pub(super) fn verify_archive(&self, bytes: &[u8]) -> crate::Result<()> {
		let expected = self
			.sha256_hash
			.as_deref()
			.filter(|hash| valid_sha256(hash))
			.context("Azul Java package has no valid SHA-256 checksum")?;
		let actual = Sha256::digest(bytes)
			.iter()
			.map(|byte| format!("{byte:02x}"))
			.collect::<String>();
		if !actual.eq_ignore_ascii_case(expected) {
			bail!("Java archive failed SHA-256 verification");
		}
		Ok(())
	}
}

fn valid_sha256(value: &str) -> bool {
	value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn validate_storage_root(root: &Path) -> crate::Result<()> {
	if !root.is_absolute() {
		bail!("managed Java storage must be absolute");
	}
	for ancestor in root.ancestors() {
		if std::fs::symlink_metadata(ancestor)?.is_symlink() {
			bail!("managed Java storage contains a symbolic link directory");
		}
	}
	Ok(())
}

pub(super) fn validate_installed_version(
	package: &Package,
	installed: &JavaVersion,
) -> crate::Result<()> {
	let expected = package
		.release()
		.context("invalid Java version in package metadata")?;
	let actual = JavaRelease::parse(&installed.version)
		.context("installed Java did not report a complete version")?;
	if installed.parsed_version != expected.version[0]
		|| actual.version != expected.version
	{
		bail!("installed Java does not match the verified package version");
	}
	Ok(())
}

pub(super) fn safe_component(name: &str) -> bool {
	!name.is_empty()
		&& !name.contains(['/', '\\', ':'])
		&& matches!(
			Path::new(name).components().next(),
			Some(Component::Normal(_))
		) && Path::new(name).components().count() == 1
}

fn legacy_name_version(name: &str) -> Option<JavaRelease> {
	static NAME: LazyLock<regex::Regex> = LazyLock::new(|| {
		regex::Regex::new(
		r"^zulu[0-9]+(?:\.[0-9]+){2,3}-ca-(?:jre|jdk)([0-9]+(?:\.[0-9]+)+)-(?:win|linux|macosx)_(?:x64|aarch64|i686|i386|aarch32hf)$"
	).expect("valid Azul package pattern")
	});
	JavaRelease::parse(NAME.captures(name)?.get(1)?.as_str())
}

fn distro_version_from_name(name: &str) -> Option<Vec<u32>> {
	let (distribution, _) = name.strip_prefix("zulu")?.split_once("-ca-")?;
	let mut version = distribution
		.split('.')
		.map(str::parse::<u32>)
		.collect::<Result<Vec<_>, _>>()
		.ok()?;
	while version.len() > 1 && version.last() == Some(&0) {
		version.pop();
	}
	Some(version)
}

fn package_is_newer(
	package: &Package,
	current: &JavaRelease,
	distro_version: &[u32],
) -> bool {
	let Some(desired) = package.release() else {
		return false;
	};
	let mut distro = package.distro_version.as_slice();
	while distro.len() > 1 && distro.last() == Some(&0) {
		distro = &distro[..distro.len() - 1];
	}
	desired > *current || (desired == *current && distro > distro_version)
}

pub(super) fn validate_archive_entry(
	name: &Path,
	directory: &str,
) -> crate::Result<()> {
	if name.to_string_lossy().contains(['\\', ':']) {
		bail!("Java archive contains an unsafe path separator");
	}
	validate_archive_components(name, directory)
}

fn validate_archive_components(
	name: &Path,
	directory: &str,
) -> crate::Result<()> {
	let mut components = name.components();
	if components.next() != Some(Component::Normal(directory.as_ref()))
		|| components
			.any(|component| !matches!(component, Component::Normal(_)))
		|| name == Path::new(directory).join(RECEIPT)
	{
		bail!("Java archive contains a path outside its package directory");
	}
	Ok(())
}

pub(super) fn validate_archive_link(
	path: &Path,
	target: &Path,
	directory: &str,
	hard: bool,
) -> crate::Result<()> {
	validate_archive_entry(path, directory)?;
	if target.is_absolute() || target.to_string_lossy().contains(['\\', ':']) {
		bail!("Java archive contains an unsafe link");
	}
	let mut resolved = if hard {
		PathBuf::new()
	} else {
		path.parent().unwrap_or(Path::new("")).to_path_buf()
	};
	for component in target.components() {
		match component {
			Component::Normal(part) => resolved.push(part),
			Component::CurDir => {}
			Component::ParentDir if resolved.components().count() > 1 => {
				resolved.pop();
			}
			_ => bail!("Java archive link escapes its package directory"),
		}
	}
	validate_archive_components(&resolved, directory)
}

pub(super) fn validate_java_path(
	directory: &Path,
	executable: &Path,
) -> crate::Result<()> {
	let root = dunce::canonicalize(directory)?;
	let resolved = dunce::canonicalize(executable)?;
	if !resolved.starts_with(&root) || resolved == root {
		bail!("Java executable points outside its package directory");
	}
	let mut parent = executable.parent();
	while let Some(path) = parent {
		if std::fs::symlink_metadata(path)?.is_symlink() {
			bail!("Java executable has a symbolic link directory");
		}
		if path == directory {
			break;
		}
		parent = path.parent();
	}
	Ok(())
}

fn cache_key(state: &State, major: u32) -> (PathBuf, u32) {
	(state.directories.java_versions_dir(), major)
}

pub(super) fn record_success(state: &State, major: u32, package: &Package) {
	if let Ok(mut checks) = CHECKS.lock() {
		checks.insert(
			cache_key(state, major),
			CheckedPackage {
				checked_at: Instant::now(),
				package: package.clone(),
			},
		);
	}
}

pub(super) fn forget_success(state: &State, major: u32) {
	if let Ok(mut checks) = CHECKS.lock() {
		checks.remove(&cache_key(state, major));
	}
}

pub(super) async fn latest_package(
	state: &State,
	major: u32,
	query: &JavaPackageQuery,
) -> crate::Result<Package> {
	if let Ok(mut checks) = CHECKS.lock() {
		if let Some(checked) = checks.get(&cache_key(state, major))
			&& checked.checked_at.elapsed() < CHECK_INTERVAL
		{
			return Ok(checked.package.clone());
		}
		checks.remove(&cache_key(state, major));
	}
	let mut url =
		url::Url::parse("https://api.azul.com/metadata/v1/zulu/packages")?;
	url.query_pairs_mut().extend_pairs([
		("arch", query.arch),
		("os", query.os),
		("archive_type", query.archive_type),
		("java_package_type", query.package_type),
		("java_version", &major.to_string()),
		("javafx_bundled", "false"),
		("crac_supported", "false"),
		("crs_supported", "false"),
		("release_status", "ga"),
		("availability_types", "CA"),
		("latest", "true"),
		("page_size", "100"),
		(
			"include_fields",
			"sha256_hash,openjdk_build_number,arch,hw_bitness",
		),
	]);
	let packages: Vec<Package> = fetch_json(
		Method::GET,
		url.as_str(),
		None,
		None,
		None,
		&state.fetch_semaphore,
		&state.pool,
	)
	.await?;
	let mut package = packages
		.into_iter()
		.filter(|package| {
			!package.crs_supported
				&& !package.crac_supported
				&& package_matches_architecture(package, query.arch)
				&& package
					.release()
					.is_some_and(|release| release.version[0] == major)
				&& package.directory_name(query.archive_type).is_ok()
		})
		.max_by_key(|package| {
			(package.release(), package.distro_version.clone())
		})
		.context(
			"Azul has no stable Java package for this platform and major version",
		)?;
	if !package.sha256_hash.as_deref().is_some_and(valid_sha256) {
		let details = format!(
			"https://api.azul.com/metadata/v1/zulu/packages/{}",
			package.package_uuid
		);
		package = fetch_json(
			Method::GET,
			&details,
			None,
			None,
			None,
			&state.fetch_semaphore,
			&state.pool,
		)
		.await?;
		package.directory_name(query.archive_type)?;
		if package
			.release()
			.is_none_or(|release| release.version[0] != major)
			|| !package.sha256_hash.as_deref().is_some_and(valid_sha256)
			|| !package_matches_architecture(&package, query.arch)
			|| package.crs_supported
			|| package.crac_supported
		{
			bail!(
				"Azul Java package details do not match the requested release"
			);
		}
	}
	Ok(package)
}

fn package_matches_architecture(package: &Package, architecture: &str) -> bool {
	match architecture {
		"x64" => package.arch == "x86" && package.hw_bitness == 64,
		"i686" => package.arch == "x86" && package.hw_bitness == 32,
		"aarch64" => package.arch == "arm" && package.hw_bitness == 64,
		"aarch32hf" => package.arch == "arm" && package.hw_bitness == 32,
		_ => false,
	}
}

#[derive(Deserialize, Serialize)]
struct Receipt {
	format: u32,
	provider: String,
	package: Package,
}

pub(super) fn write_receipt(
	directory: &Path,
	package: &Package,
) -> crate::Result<()> {
	use std::io::Write;
	let receipt = Receipt {
		format: 1,
		provider: "azul-zulu".to_owned(),
		package: package.clone(),
	};
	let mut file = std::fs::OpenOptions::new()
		.write(true)
		.create_new(true)
		.open(directory.join(RECEIPT))?;
	file.write_all(&serde_json::to_vec(&receipt)?)?;
	file.sync_all()?;
	Ok(())
}

struct OwnedJava {
	directory: PathBuf,
	executable: PathBuf,
	release: JavaRelease,
	distro_version: Vec<u32>,
}

impl OwnedJava {
	fn rank(&self) -> (&JavaRelease, &[u32]) {
		(&self.release, &self.distro_version)
	}
}

fn release_properties(path: &Path) -> crate::Result<HashMap<String, String>> {
	Ok(std::fs::read_to_string(path)?
		.lines()
		.filter_map(|line| {
			let (name, value) = line.split_once('=')?;
			Some((name.to_owned(), value.trim_matches('"').to_owned()))
		})
		.collect())
}

fn owned_distribution(
	root: &Path,
	directory: &Path,
) -> crate::Result<Option<OwnedJava>> {
	if directory.parent() != Some(root)
		|| std::fs::symlink_metadata(root)?.is_symlink()
		|| std::fs::symlink_metadata(directory)?.is_symlink()
		|| !directory.is_dir()
	{
		return Ok(None);
	}
	let Some(name) = directory.file_name().and_then(|name| name.to_str())
	else {
		return Ok(None);
	};
	let Some(named_version) = legacy_name_version(name) else {
		return Ok(None);
	};
	let distro_version = distro_version_from_name(name)
		.context("invalid Azul distribution version")?;
	#[cfg(target_os = "macos")]
	let executable = super::macos_java_executable(directory)?;
	#[cfg(not(target_os = "macos"))]
	let executable = super::java_executable(directory);
	if !executable.is_file() {
		return Ok(None);
	}
	validate_java_path(directory, &executable)?;
	let home = executable
		.parent()
		.and_then(Path::parent)
		.context("Java executable has no runtime home")?;
	let release_path = home.join("release");
	if std::fs::symlink_metadata(&release_path)?.is_symlink() {
		return Ok(None);
	}
	let properties = release_properties(&release_path)?;
	let Some(vendor) = properties.get("IMPLEMENTOR") else {
		return Ok(None);
	};
	if !vendor.contains("Azul") && !vendor.contains("Zulu") {
		return Ok(None);
	}
	let Some(mut release) = properties
		.get("JAVA_VERSION")
		.and_then(|version| JavaRelease::parse(version))
	else {
		return Ok(None);
	};
	if release.version != named_version.version {
		return Ok(None);
	}
	if let Some(runtime) = properties
		.get("JAVA_RUNTIME_VERSION")
		.and_then(|version| JavaRelease::parse(version))
		&& runtime.version == release.version
	{
		release.build = runtime.build;
	}
	let receipt_path = directory.join(RECEIPT);
	if receipt_path.exists() {
		if std::fs::symlink_metadata(&receipt_path)?.is_symlink() {
			return Ok(None);
		}
		let receipt: Receipt =
			serde_json::from_slice(&std::fs::read(&receipt_path)?)?;
		let archive = if receipt.package.name.ends_with(".tar.gz") {
			"tar.gz"
		} else {
			"zip"
		};
		if receipt.format != 1
			|| receipt.provider != "azul-zulu"
			|| receipt.package.directory_name(archive)? != name
			|| !receipt
				.package
				.sha256_hash
				.as_deref()
				.is_some_and(valid_sha256)
			|| receipt
				.package
				.release()
				.is_none_or(|version| version.version != release.version)
		{
			return Ok(None);
		}
		release.build = receipt.package.openjdk_build_number;
	}
	Ok(Some(OwnedJava {
		directory: directory.to_path_buf(),
		executable,
		release,
		distro_version,
	}))
}

fn owned_selection(
	root: &Path,
	selected: &Path,
) -> crate::Result<Option<OwnedJava>> {
	if !root.exists() {
		return Ok(None);
	}
	validate_storage_root(root)?;
	let root = match dunce::canonicalize(root) {
		Ok(root) => root,
		Err(_) => return Ok(None),
	};
	let original = selected;
	let selected = match dunce::canonicalize(selected) {
		Ok(path) => path,
		Err(_) => return Ok(None),
	};
	let Some(directory) = selected
		.strip_prefix(&root)
		.ok()
		.and_then(|relative| relative.components().next())
		.map(|part| root.join(part))
	else {
		return Ok(None);
	};
	let owned = owned_distribution(&root, &directory)?;
	if let Some(owned) = &owned {
		validate_java_path(&owned.directory, original)?;
	}
	Ok(owned)
}

fn replacement_path(old: &Path, new: &Path) -> PathBuf {
	if cfg!(windows)
		&& old
			.file_name()
			.and_then(|name| name.to_str())
			.is_some_and(|name| name.eq_ignore_ascii_case("java.exe"))
	{
		new.with_file_name("java.exe")
	} else {
		new.to_path_buf()
	}
}

fn replacement_for_selection(
	root: &Path,
	selected: &Path,
	replacement: &OwnedJava,
) -> crate::Result<Option<PathBuf>> {
	let replacement_path = owned_selection(root, selected)?
		.filter(|owned| {
			owned.release.version[0] == replacement.release.version[0]
				&& owned.rank() <= replacement.rank()
		})
		.map(|_| replacement_path(selected, &replacement.executable));
	if let Some(path) = &replacement_path {
		if !path.is_file() {
			return Ok(None);
		}
		validate_java_path(&replacement.directory, path)?;
	}
	Ok(replacement_path)
}

async fn migrate_references(
	state: &State,
	replacement: &JavaVersion,
) -> crate::Result<()> {
	let root = state.directories.java_versions_dir();
	let owned_replacement =
		owned_selection(&root, Path::new(&replacement.path))?
			.context("replacement Java is not managed")?;
	let mut tx = state.pool.begin_with("BEGIN IMMEDIATE").await?;
	let registered = JavaVersion::get_all_registered(&mut *tx).await?;
	let instance_ids =
		sqlx::query_scalar::<_, String>("SELECT id FROM instances")
			.fetch_all(&mut *tx)
			.await?;
	for entry in &registered {
		let java = entry.value();
		if java.parsed_version == replacement.parsed_version
			&& let Some(path) = replacement_for_selection(
				&root,
				Path::new(&java.path),
				&owned_replacement,
			)? {
			let mut new = replacement.clone();
			new.path = path.to_string_lossy().into_owned();
			new.upsert(&mut *tx).await?;
		}
	}
	if !registered.contains_key(&replacement.parsed_version) {
		replacement.upsert(&mut *tx).await?;
	}
	for id in instance_ids {
		if let Some(mut value) =
			instance_rows::get_instance_launch_overrides(&id, &mut *tx).await?
			&& let Some(path) = &value.java_path
			&& let Some(path) = replacement_for_selection(
				&root,
				Path::new(path),
				&owned_replacement,
			)? {
			value.java_path = Some(path.to_string_lossy().into_owned());
			instance_rows::upsert_instance_launch_overrides(&value, &mut tx)
				.await?;
		}
	}
	tx.commit().await?;
	Ok(())
}

async fn refresh_inner(
	state: &State,
	selected: &JavaVersion,
) -> crate::Result<JavaVersion> {
	let Some(owned) = owned_selection(
		&state.directories.java_versions_dir(),
		Path::new(&selected.path),
	)?
	else {
		return Ok(selected.clone());
	};
	let major = owned.release.version[0];
	let query = JavaPackageQuery::new(
		std::env::consts::OS,
		std::env::consts::ARCH,
		major,
	);
	let package = latest_package(state, major, &query).await?;
	let desired = package.release().context("invalid Java release metadata")?;
	let current = JavaRelease::parse(&selected.version).map_or_else(
		|| owned.release.clone(),
		|release| release.max(owned.release.clone()),
	);
	if !package_is_newer(&package, &current, &owned.distro_version) {
		record_success(state, major, &package);
		return Ok(selected.clone());
	}
	let path = super::install_java_for_state(
		state,
		major,
		false,
		None,
		Some(&package),
	)
	.await?;
	let mut replacement =
		crate::util::jre::check_java_at_filepath(&path).await?;
	if replacement.parsed_version != major
		|| JavaRelease::parse(&replacement.version)
			.is_none_or(|release| release.version != desired.version)
	{
		bail!("downloaded Java does not match the recommended runtime");
	}
	migrate_references(state, &replacement).await?;
	let owned_replacement =
		owned_selection(&state.directories.java_versions_dir(), &path)?
			.context("replacement Java lost its ownership metadata")?;
	let selected_path = replacement_for_selection(
		&state.directories.java_versions_dir(),
		Path::new(&selected.path),
		&owned_replacement,
	)?
	.context("replacement Java does not preserve the selected executable")?;
	replacement.path = selected_path.to_string_lossy().into_owned();
	record_success(state, major, &package);
	tracing::info!(major, previous = %selected.version, current = %replacement.version, "Updated launcher-managed Java runtime");
	Ok(replacement)
}

/// Refreshes only verified launcher-managed Java. The caller holds the runtime-cache read lease.
pub(crate) async fn refresh_managed_java(
	state: &State,
	selected: JavaVersion,
) -> JavaVersion {
	if !matches!(
		owned_selection(
			&state.directories.java_versions_dir(),
			Path::new(&selected.path)
		),
		Ok(Some(_))
	) {
		return selected;
	}
	let _update = UPDATE_LOCK.lock().await;
	match refresh_inner(state, &selected).await {
		Ok(java) => java,
		Err(error) => {
			forget_success(state, selected.parsed_version);
			tracing::warn!(path = %selected.path, "Could not update managed Java; keeping the existing runtime and launch mitigation: {error:#}");
			selected
		}
	}
}

fn installed_distributions(root: &Path) -> crate::Result<Vec<OwnedJava>> {
	if !root.exists() {
		return Ok(Vec::new());
	}
	validate_storage_root(root)?;
	let root = dunce::canonicalize(root)?;
	let mut distributions = Vec::new();
	for entry in std::fs::read_dir(&root)? {
		let path = entry?.path();
		match owned_distribution(&root, &path) {
			Ok(Some(owned)) => distributions.push(owned),
			Ok(None) => {}
			Err(error) => {
				tracing::debug!(path = %path.display(), "Keeping unrecognized Java directory: {error:#}")
			}
		}
	}
	Ok(distributions)
}

/// Refreshes all installed managed majors without entering State::get during initialization.
pub(crate) async fn refresh_managed_java_at_startup(
	state: &State,
) -> crate::Result<()> {
	let _lease = state.content_store.runtime_cache_lock.read().await;
	let mut majors: HashMap<u32, Vec<OwnedJava>> = HashMap::new();
	for java in installed_distributions(&state.directories.java_versions_dir())?
	{
		majors
			.entry(java.release.version[0])
			.or_default()
			.push(java);
	}
	for (major, mut candidates) in majors {
		candidates.sort_by(|left, right| right.rank().cmp(&left.rank()));
		for owned in candidates {
			match crate::util::jre::check_java_at_filepath(&owned.executable)
				.await
			{
				Ok(selected) => {
					let replacement =
						refresh_managed_java(state, selected).await;
					if let Err(error) =
						migrate_references(state, &replacement).await
					{
						tracing::warn!(
							major,
							"Could not update managed Java references: {error:#}"
						);
					}
					break;
				}
				Err(error) => {
					tracing::warn!(major, path = %owned.executable.display(), "Skipping invalid managed Java runtime: {error:#}")
				}
			}
		}
	}
	Ok(())
}

async fn runtime_in_use(state: &State) -> crate::Result<bool> {
	if !state.process_manager.get_all().is_empty() {
		return Ok(true);
	}
	let processes = sqlx::query_as::<_, (i64, i64)>(
		"SELECT pid, start_time FROM processes",
	)
	.fetch_all(&state.pool)
	.await?;
	let starts = crate::state::running_process_start_times(
		processes.iter().map(|(pid, _)| *pid).collect(),
	)
	.await?;
	if processes.iter().any(|(pid, start)| {
		starts
			.get(pid)
			.is_some_and(|actual| actual.abs_diff(*start) <= 2)
	}) {
		return Ok(true);
	}
	Ok(crate::install::store::list(false, &state.pool)
		.await?
		.iter()
		.any(|job| {
			matches!(
				job.status,
				crate::install::InstallJobStatus::Queued
					| crate::install::InstallJobStatus::Running
			) || job
				.state
				.paths
				.staging_dir
				.as_ref()
				.is_some_and(|path| path.exists())
		}))
}

/// Deletes superseded owned distributions only when no game or content operation can use them.
pub(crate) async fn cleanup_old_managed_java(
	state: &State,
) -> crate::Result<()> {
	let Ok(_lease) = state.content_store.runtime_cache_lock.try_write() else {
		return Ok(());
	};
	let Ok(_update) = UPDATE_LOCK.try_lock() else {
		return Ok(());
	};
	if runtime_in_use(state).await? {
		return Ok(());
	}
	let distributions =
		installed_distributions(&state.directories.java_versions_dir())?;
	let mut newest = HashMap::new();
	for java in &distributions {
		if let Ok(installed) =
			crate::util::jre::check_java_at_filepath(&java.executable).await
			&& JavaRelease::parse(&installed.version)
				.is_some_and(|version| version.version == java.release.version)
		{
			newest
				.entry(java.release.version[0])
				.and_modify(|version: &mut (JavaRelease, Vec<u32>)| {
					*version = (*version).clone().max((
						java.release.clone(),
						java.distro_version.clone(),
					))
				})
				.or_insert_with(|| {
					(java.release.clone(), java.distro_version.clone())
				});
		}
	}
	let mut protected = HashSet::new();
	let registered = JavaVersion::get_all_registered(&state.pool).await?;
	for entry in &registered {
		if let Some(owned) = owned_selection(
			&state.directories.java_versions_dir(),
			Path::new(&entry.value().path),
		)? {
			protected.insert(owned.directory);
		}
	}
	for instance in instance_rows::list_instances(&state.pool).await? {
		if let Some(overrides) = instance_rows::get_instance_launch_overrides(
			&instance.id,
			&state.pool,
		)
		.await? && let Some(path) = overrides.java_path
			&& let Some(owned) = owned_selection(
				&state.directories.java_versions_dir(),
				Path::new(&path),
			)? {
			protected.insert(owned.directory);
		}
	}
	let root = dunce::canonicalize(state.directories.metadata_dir())?;
	for java in distributions {
		if protected.contains(&java.directory)
			|| newest.get(&java.release.version[0]).is_none_or(|version| {
				(&version.0, version.1.as_slice()) <= java.rank()
			}) {
			continue;
		}
		let file = runtime_cache::RuntimeFile {
			path: java.directory.clone(),
			size: 0,
			last_used_at: 0,
			references: 0,
			directory: true,
		};
		runtime_cache::remove_file(&file, &root).await?;
		tracing::info!(path = %java.directory.display(), "Removed superseded launcher-managed Java runtime");
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_security_patch_components_and_legacy_updates() {
		assert!(
			JavaRelease::parse("21.0.13.1").unwrap()
				> JavaRelease::parse("21.0.13").unwrap()
		);
		assert!(
			JavaRelease::parse("21.0.13+11-LTS").unwrap()
				> JavaRelease::parse("21.0.13+2").unwrap()
		);
		assert_eq!(
			JavaRelease::parse("1.8.0_482-b08"),
			JavaRelease::parse("8.0.482+8")
		);
		assert!(JavaRelease::parse("21-ea").is_none());
	}

	#[test]
	fn rejects_archive_traversal_and_escaping_links() {
		for name in [
			"../zulu/bin/java",
			"/zulu/bin/java",
			"zulu/../other/java",
			"zulu\\bin\\java",
			"zulu/C:/java",
		] {
			assert!(
				validate_archive_entry(Path::new(name), "zulu").is_err(),
				"{name}"
			);
		}
		assert!(
			validate_archive_entry(Path::new("zulu/bin/java"), "zulu").is_ok()
		);
		assert!(
			validate_archive_link(
				Path::new("zulu/legal/license"),
				Path::new("../../outside"),
				"zulu",
				false
			)
			.is_err()
		);
		assert!(
			validate_archive_link(
				Path::new("zulu/legal/license"),
				Path::new("../LICENSE"),
				"zulu",
				false
			)
			.is_ok()
		);
		for name in ["", ".", "..", "C:", "a/b", "a\\b"] {
			assert!(!safe_component(name));
		}
	}

	#[test]
	fn validates_resolved_links_without_accepting_raw_windows_paths() {
		let path = Path::new("zulu/legal/license");
		assert!(
			validate_archive_link(path, Path::new("../LICENSE"), "zulu", false)
				.is_ok()
		);
		assert!(
			validate_archive_link(
				path,
				Path::new("zulu/LICENSE"),
				"zulu",
				true
			)
			.is_ok()
		);
		for target in [
			"../../outside",
			"../LICENSE\\other",
			"../C:/LICENSE",
			"../.bytelauncher-java.json",
		] {
			assert!(
				validate_archive_link(path, Path::new(target), "zulu", false)
					.is_err(),
				"{target}"
			);
		}
		assert!(
			validate_archive_link(
				path,
				Path::new("zulu/.bytelauncher-java.json"),
				"zulu",
				true
			)
			.is_err()
		);
		for source in [
			"zulu\\legal\\license",
			"zulu/legal/C:license",
			"zulu/.bytelauncher-java.json",
		] {
			assert!(
				validate_archive_link(
					Path::new(source),
					Path::new("../LICENSE"),
					"zulu",
					false
				)
				.is_err(),
				"{source}"
			);
		}
	}

	#[test]
	fn recognizes_only_provider_distribution_names() {
		assert!(
			legacy_name_version("zulu21.54.201-ca-jre21.0.13.1-win_x64")
				.is_some()
		);
		assert!(
			legacy_name_version("zulu8.90.0.19-ca-jre8.0.482-linux_x64")
				.is_some()
		);
		for name in [
			"custom-java",
			"zulu-test",
			"zulu21-ca-jre21.0.13-win_x64",
			"zulu21.54.201-ca-jre21.0.13-win_x64/../other",
		] {
			assert!(legacy_name_version(name).is_none());
		}
	}

	#[test]
	fn rejects_external_runtime_selection() {
		let temporary = tempfile::tempdir().unwrap();
		let root = dunce::canonicalize(temporary.path()).unwrap();
		let external = tempfile::tempdir().unwrap();
		let java = external.path().join("java");
		std::fs::write(&java, b"external").unwrap();
		assert!(owned_selection(&root, &java).unwrap().is_none());
	}

	#[test]
	fn ranks_fourth_component_before_vendor_and_build() {
		let mut first: Package = serde_json::from_value(serde_json::json!({ "download_url": "https://cdn.azul.com/zulu/bin/test.zip", "name": "test.zip", "package_uuid": "id", "java_version": [21,0,13], "openjdk_build_number": 99 })).unwrap();
		let mut second = first.clone();
		second.java_version.push(1);
		second.openjdk_build_number = 1;
		assert!(second.release() > first.release());
		first.java_version = vec![21, 0, 14];
		assert!(first.release() > second.release());
	}

	#[test]
	fn refreshes_newer_vendor_build_without_downgrading_java() {
		let mut package: Package = serde_json::from_value(serde_json::json!({
			"download_url": "https://cdn.azul.com/zulu/bin/runtime.zip",
			"name": "runtime.zip",
			"package_uuid": "id",
			"java_version": [21, 0, 13, 1],
			"openjdk_build_number": 2,
			"distro_version": [21, 54, 201, 0]
		}))
		.unwrap();
		let current = JavaRelease::parse("21.0.13.1+2").unwrap();
		assert!(package_is_newer(&package, &current, &[21, 54, 200]));
		assert!(!package_is_newer(&package, &current, &[21, 54, 201]));
		assert!(!package_is_newer(&package, &current, &[21, 54, 202]));
		assert_eq!(
			distro_version_from_name("zulu21.54.201.0-ca-jre21.0.13.1-win_x64"),
			Some(vec![21, 54, 201])
		);
		package.distro_version = vec![999, 999, 999];
		assert!(!package_is_newer(
			&package,
			&JavaRelease::parse("21.0.14+1").unwrap(),
			&[1]
		));
		package.openjdk_build_number = 1;
		assert!(!package_is_newer(&package, &current, &[1]));
		package.openjdk_build_number = 3;
		assert!(package_is_newer(&package, &current, &[9999]));
	}
}

#[cfg(test)]
mod safety_tests;
