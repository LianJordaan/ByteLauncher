# Maintaining ByteLauncher

ByteLauncher is an independently maintained fork of `modrinth/code`. Its
**thin patch set** keeps selected upstream merges manageable, while ByteLauncher
features and releases follow their own pace. Upstream remains available as a
reference; there is no requirement to pull or merge each Modrinth release.

The current source update adopts upstream **v0.23.1**, following the previously
shipped **v0.22.0** base. Updating source does not publish a new ByteLauncher release.

## The patch set at a glance

| Area | Files |
| --- | --- |
| Plugin system (Rust) | `apps/app/src/api/addons.rs`, built-ins in `apps/app/src/api/builtin_plugins/`, one-line hooks in `apps/app/src/api/mod.rs`, `main.rs`, `build.rs`, `capabilities/plugins.json` |
| Hide-ads native guard | one `if` at the top of `init_ads_window` in `apps/app/src/api/ads.rs` |
| Plugin loader + update banner (frontend) | `apps/app-frontend/src/plugins/plugin-loader.js`, `update-check.js`, hooks in `apps/app-frontend/src/main.js` |
| Plugins settings tab | `apps/app-frontend/src/components/ui/settings/PluginsSettings.vue`, one entry in `AppSettingsModal.vue` |
| CSP relaxation | `script-src` / `connect-src` in `apps/app/tauri.conf.json` |
| Updater feed + migration prompt | `apps/app/tauri-release.conf.json`, `apps/app/nsis/hooks.nsi` |
| CI | `.github/workflows/fork-build.yml`, `fork-release.yml`, `fork-sync.yml` |
| In-app self-updater | `fork_apply_update` in `addons.rs`; the "App updates" section of `PluginsSettings.vue` |
| Reactive plugin state | `apps/app-frontend/src/plugins/plugin-state.js` |
| Native Multi-Launch | Play-next-to-Stop in `apps/app-frontend/src/pages/instance/Index.vue`; dedupe guard removed from `packages/app-lib/src/launcher/mod.rs` |

Keeping changes isolated like this helps preserve ByteLauncher features when
merging a chosen upstream tag.

## Building

There is **no local build** here — building the Tauri app needs a full Rust +
Node toolchain. Everything builds in **GitHub Actions**:

- **`fork-build.yml`** — sanity build on app changes pushed to `main` or `sync/**`
	(and manual dispatch), so conflict resolutions can be validated before advancing `main`.
  Builds the standalone `ByteLauncher.exe` (`--no-bundle`, unsigned, no installer)
  and uploads it as an artifact. Use this to confirm a change compiles.
- **`fork-release.yml`** — builds the standalone `ByteLauncher.exe` (`--no-bundle`,
  no installer) and publishes it as a GitHub Release asset. Triggered by pushing a
  `v*` tag, or manually via **Actions → Fork release → Run workflow** with a version
	like `v0.23.1-fork.1`. To install: drop `ByteLauncher.exe` anywhere (e.g.
  `%LOCALAPPDATA%\ByteLauncher\`); it reads the shared `%AppData%\ModrinthApp\` data.
- **`fork-sync.yml`** — manual upstream sync. Merges a selected release into a
	candidate and validates a Windows build before updating `main`. Publishing a
	release is an optional checkbox, off by default. Neither sync nor release has
	a schedule.

### Inherited upstream workflows (removed)

Modrinth's own workflows (`turbo-ci`, `theseus-build`/`theseus-release`,
`frontend-deploy`/`frontend-preview`, `labrinth-docker`, `daedalus-docker`,
`prepare-pnpm-cache`, `i18n-*`, `api-client-release`, etc.) were **deleted** from
the fork. They request private **Blacksmith** runners (`blacksmith-*`) and
Modrinth-only secrets, so on a fork they queue forever ("waiting for a runner")
or fail red on every push. Only `fork-build`, `fork-release`, and `fork-sync`
remain.

`fork-sync.yml` resolves known conflicts on deleted inherited workflows by
keeping them deleted, and removes newly added upstream workflows from the
candidate. It preserves the fork's README and maintenance guide in the known
conflict cases. Unknown conflicts abort the sync and open or update a single
tracking issue; `main` stays unchanged. Resolve those conflicts locally while
preserving ByteLauncher customizations before trying again.

## Cutting a release manually

1. **Actions → Fork release → Run workflow**, enter a version (e.g. `v0.23.1-fork.1`), or
2. push a tag: `git tag v0.23.1-fork.1 && git push origin v0.23.1-fork.1`.

The build sets the app version from the tag so the in-app update banner can
compare against it.

## Syncing upstream when you choose

1. Open **Actions → Sync with upstream Modrinth → Run workflow** and select
	**main**. The workflow accepts runs from `main` only and explicitly checks out
	that branch.
2. Optionally enter an **`upstream_tag`**, such as `v0.23.1`. Leave it blank to
	select Modrinth's latest stable release. Older bases are rejected to prevent
	accidental downgrades.
3. Leave **`publish_release`** unchecked to update source only. Check it to also
	create the `v<upstream>-fork` tag and start the ByteLauncher release build after
	validation succeeds.

The workflow merges into a candidate and builds the Windows app before it
updates `main`. It verifies that `main` has not moved during validation and that
the validated candidate is the commit being published. Conflicts or a failed
build leave `main` unchanged. With release publication selected, the branch and
release tag are pushed atomically.

The upstream marker and Git ancestry identify releases already integrated into
the source. Running a source-only sync for that same base is a no-op. You can
request publication for the current base later; existing release tags are never
moved. For subsequent ByteLauncher feature releases on that base, use a new
version such as `v0.23.1-fork.1` through **Fork release**.

If a release tag exists but its build or dispatch failed, sync will leave the tag
in place. Retry **Fork release** manually on that tag with the same version.

**`RELEASE_PAT` is optional.** If present, it authenticates the push so the release
tag can trigger `fork-release.yml`. Without it, the sync uses `GITHUB_TOKEN` and
explicitly dispatches the release workflow when `publish_release` is selected.
There is no scheduled retry or automatic adoption of future upstream releases.

## Enabling the bundled auto-updater (later)

The app already self-updates via the **custom in-app updater** described above (no
signing key needed). The **bundled Tauri auto-updater** — a silent background
install from a signed NSIS package — is a separate, optional path that is **not**
enabled: it needs a minisign signing key that can only be generated by running the
Tauri CLI. Enable it only if you want Tauri's native updater in addition to the
custom one.

To turn the real updater on:

1. Generate a signing key (needs the Tauri CLI; no compiler required):
   `pnpm --filter @modrinth/app exec tauri signer generate -w minisign.key`
2. Put the **public** key in `apps/app/tauri-release.conf.json` → `plugins.updater.pubkey`.
   The endpoint there is already pointed at this fork's releases.
3. Add repo secrets `TAURI_SIGNING_PRIVATE_KEY` (the private key contents) and
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
4. In `fork-release.yml`, build with the release config and both bundles:
   `pnpm --filter=@modrinth/app run tauri build --config tauri-release.conf.json --bundles "nsis,updater"`
   and pass the two `TAURI_SIGNING_*` env vars to that step.
5. Generate an `updates.json` manifest from the produced `.nsis.zip.sig` and
   upload it to the release (see upstream `.github/workflows/theseus-release.yml`
   for the exact `jq` manifest shape).

**Never** repoint the updater endpoint back at `launcher-files.modrinth.com` —
that would auto-update the fork into official Modrinth and wipe the patches.

## Do-not-change list (migration safety)

Preserve user data across the upgrade from a real Modrinth install:

- **Bundle `identifier` = `"ModrinthApp"`** — the ONLY load-bearing constant. The
  data dir is `dirs::data_dir()/<identifier>` = `%AppData%\ModrinthApp\`, set via
  `State::init(app.config().identifier.clone())`. Never rename it.
- No cleanup of the data directory anywhere.
- `productName`/`mainBinaryName` = `"ByteLauncher"` (rebranded from `Modrinth App`
  for Modrinth-terms compliance → `ByteLauncher.exe`). These do NOT affect the data
  dir, so the rename kept zero data loss — do not restore the Modrinth trademark.

Everything else (display name, icons, about text, exe name, theme, repo name) is
rebranded to ByteLauncher; only the `identifier` must stay `ModrinthApp`.

## Plugin system internals

- Plugins live in `%AppData%\ModrinthApp\plugins\<id>\` as a `manifest.json` plus
  `.js`/`.css`. Enabled state persists in `plugins\enabled.json` (separate from
  the plugin files, so updating a built-in never resets the user's toggles).
- The Rust `addons` plugin (`read_plugins`, `set_plugin_enabled`, `get_plugins_dir`,
  `fork_apply_update`) reads/writes them. The frontend loader (`plugin-loader.js`)
  injects each enabled plugin's CSS as a `<style>` and runs its JS via **indirect
  eval** (`script-src 'unsafe-eval'`; a Tauri CSP nonce neutralizes `'unsafe-inline'`
  and `blob:` scripts, but not eval). It also populates the reactive
  `enabledPluginIds` set (`plugin-state.js`) that native features read.
- Built-in plugins are embedded via `include_str!` and seeded on first run. To add
  one, drop a folder under `apps/app/src/api/builtin_plugins/` and add it to the
  `BUILTINS` array in `addons.rs`. `preserve: true` files (custom-css's `user.css`)
  are never overwritten once the user has them.
- **Hide-ads** is enforced natively: `init_ads_window` (`ads.rs`) returns early when
  the plugin is enabled, so the ad webview is never created.
- **Multi-Launch** is native and manifest-only (no JS): `Index.vue` shows a Play
  button next to Stop when `playing && enabledPluginIds.has('multi-launch')`, and
  the "already running" guard was removed from `launcher/mod.rs`. Accidental
  double-launch is blocked by disabling Play while a launch is in flight.

## In-app self-updater

The app updates itself without an installer. `fork_apply_update(download_url,
expected_sha256)` in `addons.rs`: validates the URL (HTTPS + GitHub hosts only),
downloads the new exe, checks size + `MZ` header + GitHub's published SHA-256
digest, renames the running exe to `ByteLauncher.old.exe`, moves the new one into
place (with a copy-fallback so the app is never left without an exe), then
`app.restart()`s. Startup deletes any stale `*.old.exe`. The **Settings → Plugins →
App updates** UI checks the fork's latest release, skips installer assets, offers
only strictly-newer versions, and passes the asset's digest. This is independent
of the optional bundled Tauri updater below.

### Update-check endpoint — Cloudflare Worker (v0.15.11-fork.1+)

The update **check** (both the Settings UI and the startup banner in
`update-check.js`) fetches its release manifest from a Cloudflare Worker at
`https://cloudflare-api.bytebuilders.co.za/bytelauncher/`, not `api.github.com`.
Why: GitHub's API is 60 requests/hr per IP, which users behind a shared IP kept
exhausting, so update prompts silently stopped appearing. The Worker calls GitHub
**server-side** (Cloudflare's IP) and **edge-caches** the response ~5 min, so all
users share one lookup and the per-user limit is never in play.

- **Worker source:** `.private/update-worker.js` (gitignored, NOT shipped — deploy
  it by hand). It's a single module Worker; paste it as the Worker's `src/index.js`.
- **Route:** `cloudflare-api.bytebuilders.co.za/bytelauncher/*` (single-label
  subdomain → free `*.bytebuilders.co.za` Universal SSL; plural `bytebuilders`,
  not the hosting proxy's singular `bytebuilder.co.za`).
- **Manifest shape:** a trimmed copy of GitHub's `releases/latest` object
  (`tag_name`, `html_url`, `assets[].name/.browser_download_url/.digest/...`), so
  the frontend parsing didn't change. It backfills the exe `digest` from the
  `ByteLauncher.exe.sha256` sidecar when GitHub's `digest` is null.
- **Optional secret:** `wrangler secret put GITHUB_TOKEN` (a zero-scope / public-repo
  read-only PAT) lifts the Worker's server-side GitHub limit to 5000/hr. With the
  cache this is rarely needed — pure insurance.
- **No Rust change:** the exe still downloads from `github.com`, already in
  `fork_apply_update`'s host allowlist. Switching the check only touched the two
  frontend fetch URLs and the `connect-src` CSP in `tauri.conf.json`.
- **Degrades safely:** if the Worker is down/misrouted, both call sites treat a
  non-2xx as "skip the check" — the app never breaks, it just won't detect updates
  until the Worker is reachable. Delivery of a build is unaffected (users still
  arrive via the prior build's check + the `github.com` download).
