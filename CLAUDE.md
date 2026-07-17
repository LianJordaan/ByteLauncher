# ByteLauncher — fork of the Modrinth App (READ THIS FIRST)

This repository is **ByteLauncher**, a personal fork of `modrinth/code` (the Modrinth monorepo — upstream's own docs begin at the `# Modrinth Monorepo` heading further down). It bakes a **Vencord-style plugin system**, an **in-app self-updater**, and **automatic upstream syncing** into the Modrinth desktop app (`apps/app` + `apps/app-frontend`).

- **Owner:** GitHub **LianJordaan**. **Fork repo:** `git@github.com:LianJordaan/ByteLauncher.git` (public). **Upstream remote:** `https://github.com/modrinth/code.git`.
- **The user is a capable hobbyist, not a Rust/Vue expert — explain things clearly and don't assume deep framework knowledge.**
- The authoritative engineering reference is **[MAINTAINING.md](MAINTAINING.md)**; the user-facing overview is **[README.md](README.md)**. Keep all three in sync.

## North star

A "Vencord for the Modrinth App": a forked Modrinth desktop app (Tauri v2 + WebView2) with a built-in plugin system (toggle in-app: hide ads, run the same instance multiple times, custom CSS/themes) that:

- loads plugins on every launch with **no debug port and no background process**;
- ships as a **standalone `ByteLauncher.exe`** (no installer) that reads the **same `%AppData%\ModrinthApp\` data dir** as an existing Modrinth install (via the unchanged bundle `identifier`), so instances/accounts/settings carry over with **zero data loss**;
- **stays synced with upstream automatically** and **updates itself in-app**.

## Working conventions (IMPORTANT — read before acting)

- **No local builds.** This machine has no C/C++ linker and no set-up Node/pnpm toolchain, so the Tauri app **cannot be built locally**. Everything builds in **GitHub Actions**. Never run `tauri build` / `cargo build` / `pnpm build` locally.
- **Don't install or run things on the user's PC without asking.** Git ops and writing files are fine; building/running the app or installing toolchains needs explicit OK.
- **`gh` CLI is NOT installed.** Use plain `git` (SSH auth works as LianJordaan) or the GitHub REST API via `curl`/Python (unauthenticated = 60 req/hr).
- **Keep the patch set thin** (new files + a few one-line hooks) so `git merge <upstream-tag>` rarely conflicts.
- **Ship carefully:** the proven loop is *write code → run a Workflow of review agents (compile / vue-tsc / logic / safety) → fix findings → commit → tag `v0.15.10-fork.N` → poll the CI run until it concludes*. Risky native code (exe swap, launcher changes) gets adversarially reviewed before tagging.
- **Indentation:** Rust files use **4 spaces** (rustfmt); frontend (JS/Vue) uses **tabs**. (The upstream "use TAB everywhere" note below is about the frontend.)

## Current status (latest release: v0.15.11-fork.1)

Everything works end-to-end and is shipping. The app is fully **rebranded to ByteLauncher** (Modrinth branding removed per `COPYING.md` — see the Branding section) and defaults to a purple theme. Releases ship a standalone `ByteLauncher.exe` **plus** two NSIS installers (`ByteLauncher-Setup.exe` online / `ByteLauncher-Setup-Offline.exe`); the app updates itself in-app — one-click from the Settings page **and** the startup banner. Recent fork.N history: fork.7 removed the core "already running" guard so relaunch works; **fork.8 = the ByteLauncher de-brand** (name/exe/icons/theme; bundle `identifier` kept `ModrinthApp` for zero data loss); fork.9 added the Purple theme (default) + fixed green accents; fork.10 = ByteBuilders Hosting "coming soon" page + more green fixes; **fork.11 = one-click update banner + the online/offline installers**; **fork.19 = two new opt-in built-in plugins: Hide News + Home Dashboard (Experimental)**; **fork.20 = the ByteBuilders Hosting (Experimental) plugin — the panel is back, now as an iframe through a header-stripping proxy (no native webview → no overlay bug) with a Main/SuperS/Kia panel picker** (see the Built-in plugins list); **fork.21 = fix panel login ("CSRF token mismatch") by disabling WebView2 tracking prevention on the main webview so third-party iframe cookies work**; **fork.22 = move the hosting proxy to subdomain-per-panel so the panel SPA routes at root and no longer 404s on tab-reopen**; **fork.23–24 = single-label proxy subdomains for free SSL; landed on a dedicated `<id>.bytebuilder.co.za` domain (fork.24)**; **fork.25 = Hide-News "hide entire section" option; fork.26 = plugin themes (plugins can add a selectable color theme) + the Rebrand Template builtin**. (fork.12–18 were DB self-heal, purple-default fix, installer download progress/rate-limit fixes, the Uninstall danger-zone, and reverting Hosting to "Coming soon".) After fork.26 the daily sync merged upstream **v0.15.11** (so the fork.N counter restarted on that base): **v0.15.11-fork.1 = route the in-app update *check* through a Cloudflare Worker** (`https://cloudflare-api.bytebuilders.co.za/bytelauncher/`) instead of `api.github.com`, so users can never hit GitHub's 60/hr API rate limit — see the In-app self-updater section.

**Two optional follow-ups that need the user** (see MAINTAINING.md):
1. **Bundled Tauri auto-updater** — *not* enabled (needs a minisign signing key generated via the Tauri CLI). The custom in-app updater already covers updates, so this is optional.
2. **`RELEASE_PAT` repo secret** — lets `fork-sync` auto-*publish* releases (the merge + fork-tag already happen automatically without it; only the release build needs it, because the default `GITHUB_TOKEN` can't trigger another workflow).

## What's built — feature map

**Plugin system (Rust `addons` Tauri plugin):** `apps/app/src/api/addons.rs` — commands `read_plugins`, `set_plugin_enabled`, `get_plugins_dir`, `fork_apply_update`. Registered via one-liners in `apps/app/src/api/mod.rs`, `main.rs`, `build.rs`, and `capabilities/plugins.json` (`addons:default`). Plugins live in `%AppData%\ModrinthApp\plugins\<id>\` (`manifest.json` + `.js`/`.css`); enabled state in `plugins\enabled.json` (separate, so updating a built-in never resets toggles). Built-ins are embedded via `include_str!` from `apps/app/src/api/builtin_plugins/` and seeded on first run; `preserve: true` files (custom-css `user.css`) are never overwritten.

**Frontend loader:** `apps/app-frontend/src/plugins/plugin-loader.js` (run from `main.js` after mount) injects each enabled plugin's CSS as a `<style>` and runs its JS via **indirect eval** (`script-src 'unsafe-eval'` — a Tauri CSP nonce can't neutralize eval, unlike inline/blob scripts). `apps/app-frontend/src/plugins/plugin-state.js` holds a reactive `enabledPluginIds` set (populated by the loader, updated live on toggle) that native features read to gate themselves. **Settings → Plugins** tab: `apps/app-frontend/src/components/ui/settings/PluginsSettings.vue` + one entry in `AppSettingsModal.vue`.

**Built-in plugins:**
- **Hide Ads** (on by default) — CSS hides `.ad-parent`, the Modrinth+ upsell (`a[href="https://modrinth.plus?app"]`), and the sidebar fade (`.app-sidebar::after`); PLUS a native guard in `apps/app/src/api/ads.rs` `init_ads_window` that returns early when enabled, so the native ad webview is never created (`crate::api::addons::is_plugin_enabled("hide-ads")`, cfg'd `not(target_os = "linux")`).
- **Multi-Launch** (opt-in, native — manifest-only, no JS) — when `playing && enabledPluginIds.has('multi-launch')`, `apps/app-frontend/src/pages/instance/Index.vue` shows a native **Play** button next to **Stop** to launch another copy. The core dedupe guard in `packages/app-lib/src/launcher/mod.rs` (which rejected launching an already-running instance) was **removed**; accidental double-fire is prevented by disabling Play while a launch is in flight.
- **Custom CSS** (opt-in) — loads user CSS from `plugins\custom-css\user.css`.
- **Hide News** (opt-in, native — manifest-only) — right-click a news article in the right sidebar to hide it. `apps/app-frontend/src/components/ui/NewsPanel.vue` (extracted from App.vue, fed the `news` array; App.vue now slices 12 not 4 so hiding backfills) uses the shared `ContextMenu`; the "News" heading gains an eye button (show/restore hidden, with per-article Unhide badges) + a restore-all button. Hidden IDs live in `localStorage` (`bytelauncher-hidden-news`) via `apps/app-frontend/src/plugins/hidden-news.js` — never the shared `app.db`. Gated by `enabledPluginIds.has('hide-news')`.
- **Home Dashboard (Experimental)** (opt-in, native — manifest-only) — when `enabledPluginIds.has('experimental-home')`, `apps/app-frontend/src/pages/Index.vue` renders `HomeDashboard.vue` (big quick-launch `HomeQuickLaunchTile.vue` hero tiles for recent instances + RecentWorldsList + Discover rows) instead of the default home; else unchanged. The tile reuses Instance.vue's proven run/kill/process_listener logic (listener registered in `onMounted`, so the tile stays synchronous).
- **Rebrand Template** (opt-in, built-in) + **plugin themes** (fork.26) — a plugin can contribute a selectable color theme to Settings → Appearance by adding `"theme": "<Name>"` to its manifest and a `.<Name>-mode { … }` CSS block. The frontend loader collects enabled plugins' themes and calls `themeStore.setPluginThemes(...)` (`apps/app-frontend/src/store/theme.ts`); a plugin theme layers over the built-in `dark-mode` base (so templates only override the accent), and `getThemeOptions()` feeds them into the existing `<ThemeSelector>`. `ColorTheme` was broadened to `string`; unknown/disabled themes fall back to dark. The **Rebrand Template** builtin (`builtin_plugins/rebrand-template/`, `theme.css` is `preserve: true`) is an editable starter ("MyBrand" theme). The private **Restore Modrinth** plugin lives in `.private/modrinth-restore/` (gitignored, NOT shipped) and adds a "Modrinth" theme that restores the green accent **and the Modrinth wordmark logo** (the statusbar logo carries a stable `app-brand-logo` class; the theme swaps it via `content: url("data:image/svg+xml;base64,…")` using Modrinth's original `modrinth_app.svg` recovered from git — this is the pattern for a theme swapping any in-app image). **Native limits:** the window title and taskbar/exe icon are compiled in and cannot be changed by a plugin. The **splash screen** renders before plugins load, so theming it needs a boot-CSS cache (inject the active theme's CSS before `app.mount()`) — not yet built.
- **ByteBuilders Hosting (Experimental)** (opt-in, native — manifest-only) — when `enabledPluginIds.has('hosting')`, the `/hosting/manage/` page (`apps/app-frontend/src/pages/Servers.vue`) renders the game panel in a plain **`<iframe>`** (regular DOM, so it sits *under* modals/tooltips — the fix for the old native-webview overlay bug). The panel is loaded through a header-stripping Cloudflare-Worker proxy. Each panel is served at the **root of its own subdomain** on a dedicated proxy domain (`https://<id>.bytebuilder.co.za/`, id = main/supers/kia; the Worker maps the subdomain's first label → the real panel host — the panels live on `bytebuilders.co.za`/`supersnetwork.com`, note the singular vs plural). Single-label subdomains are covered by the free `*.bytebuilder.co.za` Universal SSL cert (a two-level `<id>.proxy.bytebuilders.co.za` had no free cert → `ERR_SSL_VERSION_OR_CIPHER_MISMATCH`, which is why a separate domain was bought — fork.24). Root-hosting — NOT a `/<host>` path prefix — is required: the panels are single-page apps, so a path prefix makes their router 404 on `/<host>` when the tab reopens (fork.22). The proxy strips `X-Frame-Options`/CSP and rewrites every cookie to `SameSite=None; Secure`; `frame-src` in `apps/app/tauri.conf.json` allows `https://*.bytebuilder.co.za`. The selected panel (Main `panel.bytebuilders.co.za` / SuperS Network / Kia) lives in `localStorage` via `apps/app-frontend/src/plugins/hosting-panel.js` (each panel carries its `url`) and is chosen in a per-plugin settings block in `PluginsSettings.vue` (shown when the plugin is enabled). The old dormant `set_hosting_webview`/`reload_hosting_webview` Rust commands are unused by this path (left registered, harmless). **WebView2 gotcha:** the panel is a Laravel/Pterodactyl app whose login needs a session cookie, which is *third-party* inside the iframe. WebView2's tracking prevention (default Balanced) blocks third-party cookies → "CSRF token mismatch" on login. `main.rs` setup calls `crate::api::ads::disable_tracking_prevention` (`SetPreferredTrackingPreventionLevel(NONE)`, the same call ads.rs already used for the ad webview — which never runs when Hide Ads is on) on the **main** webview to allow them. The proxy must also emit cookies as `SameSite=None; Secure` (Lax/Strict are dropped in a cross-site iframe regardless).

**In-app self-updater:** `fork_apply_update(app, download_url, expected_sha256)` in `addons.rs`. Validates the URL (HTTPS + GitHub hosts only), downloads via `tauri_plugin_http::reqwest`, verifies size + `MZ` header + GitHub's published SHA-256 digest, then swaps (rename running exe → `ByteLauncher.old.exe`, move new exe in with a copy-fallback rollback so the app is never left without an exe), `app.restart()`. Startup cleans up `*.old.exe`. Frontend: the "App updates" section of `PluginsSettings.vue`. Startup version banner: `apps/app-frontend/src/plugins/update-check.js` — a dismissible **one-click "Download & install"** banner that runs the same `fork_apply_update` path (falls back to "View release" if a release has no verifiable digest). **Update *check* endpoint (v0.15.11-fork.1+):** both the banner and the Settings page fetch the release manifest from a **Cloudflare Worker** at `https://cloudflare-api.bytebuilders.co.za/bytelauncher/` (source in `.private/update-worker.js`, gitignored, NOT shipped) instead of `api.github.com`. The Worker mirrors GitHub's `releases/latest` JSON shape (so the frontend parsing is unchanged), calls GitHub **server-side** (Cloudflare's IP, optionally with a `GITHUB_TOKEN` secret) and **edge-caches** the result for 5 min, so users can never hit GitHub's 60/hr API rate limit (the recurring cause of missed update prompts). It also backfills the exe's SHA-256 `digest` from the published `ByteLauncher.exe.sha256` sidecar when GitHub's `digest` field is null. The **download** still comes from `github.com` (already in `fork_apply_update`'s host allowlist), so there was **no Rust change** — only the two frontend fetch URLs plus adding the Worker host to `connect-src` in `tauri.conf.json`. Worker route: `cloudflare-api.bytebuilders.co.za/bytelauncher/*` (a single-label subdomain, covered by the free `*.bytebuilders.co.za` Universal SSL — note plural `bytebuilders`, distinct from the hosting proxy's singular `bytebuilder.co.za`).

**Installers (`fork/`, built in CI):** two NSIS installers (`fork/installer/bytelauncher-installer.nsi`, online + offline via `-DOFFLINE`) that install ByteLauncher **on top of an existing Modrinth App**. They require `%LOCALAPPDATA%\Modrinth App\Modrinth App.exe` (abort with an explanation otherwise), close the app if running, place `ByteLauncher.exe` (online: downloads the latest via GitHub's **non-API** `releases/latest/download` redirect (NOT rate-limited `api.github.com` — the API broke a first install) and verifies a published `ByteLauncher.exe.sha256` sidecar, see `fork/installer/download.ps1`; offline: bundled), back up the original `Modrinth App.exe` → `Modrinth App.old.exe` (size-guarded so re-runs never clobber the real backup), and install a tiny **Rust shim** (`fork/shim/` — a standalone crate *outside* the cargo workspace, with an empty `[workspace]`) as `Modrinth App.exe` that launches `ByteLauncher.exe` forwarding args, so existing shortcuts keep working. Also adds a `ByteLauncher` Start Menu shortcut and an **uninstaller** (registered in Add/Remove Programs) that restores `Modrinth App.exe` from the `.old.exe` backup and reverts cleanly to Modrinth. Published as `ByteLauncher-Setup.exe` + `ByteLauncher-Setup-Offline.exe` (the raw `ByteLauncher.exe` stays too — the in-app updater needs it). `makensis` is invoked from a `pwsh` CI step (git-bash mangles `/D` flags); `fork-build.yml` compiles both installers on push to validate.

**CI:** `.github/workflows/fork-build.yml` (validate — builds the standalone exe on push), `fork-release.yml` (build + publish the exe on a `v*` tag / dispatch), `fork-sync.yml` (daily; merges new upstream **releases** into `main`, tags `v<upstream>-fork`). All inherited Modrinth workflows were **deleted** (they need private Blacksmith runners / Modrinth secrets and just hang or fail on a fork).

## Migration safety — do NOT change

- **Bundle `identifier` = `"ModrinthApp"`** (in `apps/app/tauri.conf.json`) is the load-bearing constant: the data dir is `dirs::data_dir()/<identifier>` = `%AppData%\ModrinthApp\` (instances/accounts/settings), wired via `State::init(app.config().identifier.clone())`. **Never change `identifier`** and never add data-dir cleanup — that is what preserves user data.
- `productName`/`mainBinaryName` are `"ByteLauncher"` (compliance rebrand — exe `ByteLauncher.exe` in `%LOCALAPPDATA%\ByteLauncher\`). These are **independent of the data dir**, so the rename kept zero data loss. Do **not** "restore" them to `Modrinth App` — that re-introduces the Modrinth trademark the fork must not use.
- The updater endpoint in `apps/app/tauri-release.conf.json` points at the fork's releases — **never** repoint it to `launcher-files.modrinth.com` (that would auto-update the fork back into real Modrinth).
- **Never add fork-only DB migrations.** `app.db` is **shared** with the real Modrinth App (same identifier), so a fork-only migration writes a `_sqlx_migrations` row upstream doesn't recognize → Modrinth (or a fork build that later drops it) refuses to start: *"migration X was previously applied but is missing in the resolved migrations."* Do fork-specific data changes in **app logic**, not migrations — e.g. default-purple is set in `App.vue` `setupApp()` on fresh (not-yet-onboarded) installs, and writes only a settings *value* (`theme='purple'`, which Modrinth safely reads as its Dark fallback). `packages/app-lib/src/state/db.rs` `remove_fork_only_migrations()` self-heals by deleting the one legacy bad row (`20260712120000`) before `migrate()`. (This bit a user in fork.9–11; fixed in fork.12.)

## Branding (ByteLauncher rebrand — compliance)

Modrinth's `COPYING.md` requires forks to remove all Modrinth branding assets (logos, landing art, trademark). What changed vs. upstream:

- **Name/identity:** product/exe/window title = **ByteLauncher**; app icons + in-app logo/wordmark + splash = a hexagon-**B** mark (master `apps/app/icons/bytelauncher.svg`; OS icons in `apps/app/icons/` regenerated by a Pillow script from that mark; in-app vectors `apps/app-frontend/src/assets/bytelauncher_logo.svg` + `bytelauncher_mark.svg`). Removed the Modrinth logo/mascot (`sad-modrinth-bot.webp`), the Modrinth macOS `.icon` bundle, and the `.github/assets/*_cover.png` banners; every raster icon (incl. `icon.icns`) is regenerated as the ByteLauncher mark.
- **Theme accent → purple** (`--color-brand` = `#6c4bff` dark / `#5a38d6` light in `packages/assets/styles/variables.scss`); the green scale stays as the **success** semantic; `.btn-primary` text forced white for contrast on purple.
- **Kept (functional, not branding):** Modrinth API/CDN hosts, `modrinth://` deep-link, `.mrpack`, mod-repo browsing, the `com.modrinth.theseus` codename, and an honest "Built on Modrinth" line (About tab). The bundle `identifier`/data-dir folder stays `ModrinthApp` (internal, not user-facing — see Migration safety).
- **Scope:** only the shipped desktop app (`apps/app` + `apps/app-frontend`) and the shared packages it compiles (`app-lib`, `ui`, `assets`) were rebranded. The website (`apps/frontend`), `apps/docs`, `packages/blog` (Modrinth's authored posts), and `packages/moderation` are **not built/shipped by the fork** and are left as-is.

## Gotchas / hard-won lessons

- **CSP nonce:** Tauri injects a nonce that neutralizes `'unsafe-inline'` and can block `blob:` scripts, so plugin JS uses **indirect eval** (`'unsafe-eval'` is honored regardless). CSP is relaxed in `apps/app/tauri.conf.json` (`script-src`, `connect-src` + `https://api.github.com`).
- **The in-memory launch guard** in `launch_minecraft` is why multi-launch only worked after restarting the app (a fresh process list). Removed in fork.7.
- **Exe name has no space now (`ByteLauncher.exe`),** so GitHub no longer mangles the release-asset name (the old `Modrinth App.exe` downloaded as `Modrinth.App.exe`). Drop it into `%LOCALAPPDATA%\ByteLauncher\`; user data stays in `%AppData%\ModrinthApp\` via the unchanged `identifier`.
- **Apply timing:** CSS/JS plugins apply on next launch (restart-to-apply); the native Multi-Launch applies live via the reactive set.
- **Project memory does NOT travel** with a directory move (it's keyed to the project path under `~/.claude`). This CLAUDE.md + MAINTAINING.md + README.md are the authoritative, self-contained record — keep them current.

---

# Modrinth Monorepo

This is the Modrinth monorepo — it contains all Modrinth projects, both frontend and backend. When entering a project, either to edit or analyse, you should read it's CLAUDE.md.

## Architecture

- **Monorepo tooling:** [Turborepo](https://turbo.build/) (`turbo.jsonc`) + [pnpm workspaces](https://pnpm.io/workspaces) (`pnpm-workspace.yaml`)
- **Frontend:** Vue 3 / Nuxt 3, Tailwind CSS v3
- **Backend:** Rust (Labrinth API), Postgres, Clickhouse
- **Indentation:** Use TAB everywhere, never spaces

### Apps (`apps/`)

| App               | Description                    |
| ----------------- | ------------------------------ |
| `frontend`        | Main Modrinth website (Nuxt 3) |
| `app-frontend`    | Desktop/app frontend (Vue 3)   |
| `app`             | Desktop/app shell (Tauri)      |
| `app-playground`  | Testing playground for app     |
| `labrinth`        | Backend API service            |
| `daedalus_client` | Daedalus client implementation |
| `docs`            | Documentation site (Astro)     |

### Packages (`packages/`)

| Package            | Description                                           |
| ------------------ | ----------------------------------------------------- |
| `ui`               | Shared Vue component library (`@modrinth/ui`)         |
| `assets`           | Styling and auto-generated icons (`@modrinth/assets`) |
| `api-client`       | API client for Nuxt, Tauri, and Node/browser          |
| `app-lib`          | Shared app library                                    |
| `blog`             | Blog system and changelog data                        |
| `utils`            | Shared utility functions (mostly deprecated)          |
| `moderation`       | Moderation utilities                                  |
| `daedalus`         | Daedalus protocol                                     |
| `tooling-config`   | ESLint, Prettier, TypeScript configs                  |
| `ariadne`          | Analytics library                                     |
| `modrinth-log`     | Logging utilities                                     |
| `modrinth-maxmind` | MaxMind GeoIP                                         |
| `modrinth-util`    | General utilities                                     |
| `muralpay`         | Payment processing                                    |
| `path-util`        | Path utilities                                        |
| `sqlx-tracing`     | SQLx query tracing                                    |

## Pre-PR Commands

Run these from the **root** folder before opening a pull request - do not run these after each prompt the user gives you, only run when asked, ask the user a question if they want to run it if the user indicates that they are about to create a pull request.

- **Website:** `pnpm prepr:frontend:web`
- **App frontend:** `pnpm prepr:frontend:app`
- **Frontend libs:** `pnpm prepr:frontend:lib`
- **All frontend (app+web):** `pnpm prepr`
- **Labrinth (backend):** See `apps/labrinth/AGENTS.md`

The website and app `prepr` commands

## Dev Commands

- **Website:** `pnpm web:dev` (copy `.env` template in `apps/frontend/` first)
- **App:** `pnpm app:dev` (copy `.env` template in `packages/app-lib/` first)
- **Storybook (packages/ui):** `pnpm storybook`

## Project-Specific Instructions

Each project may have its own file with detailed instructions:

- [`apps/labrinth/AGENTS.md`](apps/labrinth/AGENTS.md) — Backend API
- [`apps/frontend/CLAUDE.md`](apps/frontend/CLAUDE.md) - Frontend Website

## Code Guidelines

### Comments
- DO NOT use "heading" comments like: `=== Helper methods ===`.
- Use doc comments, but avoid inline comments unless ABSOLUTELY necessary for clarity. Code should aim to be self documenting!

## Bash Guidelines

### Output handling
- DO NOT pipe output through `head`, `tail`, `less`, or `more`
- NEVER use `| head -n X` or `| tail -n X` to truncate output
- IMPORTANT: Run commands directly without pipes when possible
- IMPORTANT: If you need to limit output, use command-specific flags (e.g. `git log -n 10` instead of `git log | head -10`)
- ALWAYS read the full output — never pipe through filters

### General
- Do not create new non-source code files (e.g. Bash scripts, SQL scripts) unless explicitly prompted to
- For Frontend, when doing lint checks, only use the `prepr` commands, do not use `typecheck` or `tsc` etc.
- Types in `@modrinth/utils` are considered highly outdated, if a component needs them, check if you can switch said component to use types from `packages/api-client`
- When provided problems, do not say "I didn't introduce these problems" (shifting the blame/effort) - just fix them.

## Edit Tool - Whitespace Handling (CLAUDE ONLY)

The Read tool uses `→` to mark where line numbers end and file content begins.

**Rule:** Copy the EXACT whitespace that appears after the `→` marker.
- Whatever appears between `→` and the code text is what's actually in the file
- That whitespace must be used EXACTLY in Edit tool's old_string
- Don't count arrows, don't interpret - just copy what's after the `→`

**Example:**
14→		private byte tag;
For Edit, use: `		private byte tag;` (copy everything after →, including the two tabs)

**If Edit fails:** Stop and explain the problem. Do not attempt sed/awk/bash workarounds.

**IMPORTANT**: Trust the Read tool output. Copy what's after `→` into Edit immediately. DO NOT verify with sed/od/grep first - that's wasting time and the instructions already tell you to stop if Edit fails, not to pre-verify.

## Standards

Standards available at the @standards/ folder.
