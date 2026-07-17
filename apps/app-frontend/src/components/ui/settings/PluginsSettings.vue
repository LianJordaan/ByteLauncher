<script setup lang="ts">
import { SearchIcon } from '@modrinth/assets'
import { ButtonStyled, Toggle } from '@modrinth/ui'
import { getVersion } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { computed, ref } from 'vue'

import { openPath, restartApp } from '@/helpers/utils'
import { hideAllNews, setHideAllNews } from '@/plugins/hidden-news'
import {
	HOSTING_PANELS,
	selectedPanel,
	selectedPanelId,
	setHostingPanel,
} from '@/plugins/hosting-panel'
import { setPluginEnabledState } from '@/plugins/plugin-state'

interface PluginData {
	id: string
	name: string
	description: string
	version: string
	author: string
	enabled: boolean
	builtin: boolean
	js: string | null
	css: string | null
	updatedAt: number
}

const plugins = ref<PluginData[]>((await invoke('plugin:addons|read_plugins')) as PluginData[])
const pluginsDir = ref<string>((await invoke('plugin:addons|get_plugins_dir')) as string)
const restartNeeded = ref(false)

type PluginFilter = 'all' | 'enabled' | 'disabled' | 'new'
type PluginSort = 'name' | 'recent'
const PLUGIN_FILTERS: { id: PluginFilter; label: string }[] = [
	{ id: 'all', label: 'All' },
	{ id: 'enabled', label: 'Enabled' },
	{ id: 'disabled', label: 'Disabled' },
	{ id: 'new', label: 'New' },
]
const PLUGIN_SORTS: { id: PluginSort; label: string }[] = [
	{ id: 'name', label: 'Name' },
	{ id: 'recent', label: 'Recently updated' },
]
const searchQuery = ref('')
const filterMode = ref<PluginFilter>('all')
const sortMode = ref<PluginSort>('name')

// A plugin counts as "new" for a week after its files were last written, so a
// freshly-added or just-updated plugin is easy to spot.
const NEW_PLUGIN_WINDOW_MS = 7 * 24 * 60 * 60 * 1000
function isNewPlugin(plugin: PluginData): boolean {
	return plugin.updatedAt > 0 && Date.now() - plugin.updatedAt < NEW_PLUGIN_WINDOW_MS
}

const filteredPlugins = computed(() => {
	const query = searchQuery.value.trim().toLowerCase()
	const list = plugins.value.filter((plugin) => {
		if (filterMode.value === 'enabled' && !plugin.enabled) return false
		if (filterMode.value === 'disabled' && plugin.enabled) return false
		if (filterMode.value === 'new' && !isNewPlugin(plugin)) return false
		if (!query) return true
		return (
			plugin.name.toLowerCase().includes(query) ||
			plugin.description.toLowerCase().includes(query) ||
			plugin.id.toLowerCase().includes(query) ||
			plugin.author.toLowerCase().includes(query)
		)
	})
	return list.sort((a, b) =>
		sortMode.value === 'recent'
			? b.updatedAt - a.updatedAt
			: a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
	)
})

async function toggle(plugin: PluginData) {
	const next = !plugin.enabled
	plugin.enabled = next
	restartNeeded.value = true
	setPluginEnabledState(plugin.id, next)
	try {
		await invoke('plugin:addons|set_plugin_enabled', { id: plugin.id, enabled: next })
	} catch (e) {
		console.error('Failed to toggle plugin', e)
		plugin.enabled = !next
		setPluginEnabledState(plugin.id, !next)
	}
}

async function openFolder() {
	if (pluginsDir.value) {
		await openPath(pluginsDir.value)
	}
}

// ByteLauncher update endpoint (Cloudflare Worker). It mirrors GitHub's
// `releases/latest` JSON shape but is edge-cached and server-side authenticated,
// so users never hit GitHub's 60/hr API rate limit. The .exe still downloads
// from github.com (already trusted by the Rust updater).
const UPDATE_API = 'https://cloudflare-api.bytebuilders.co.za/bytelauncher/'
type UpdateState = 'idle' | 'checking' | 'current' | 'available' | 'installing' | 'error'
interface GithubAsset {
	name?: string
	browser_download_url?: string
	digest?: string | null
}
const currentVersion = ref<string>(await getVersion())
const updateState = ref<UpdateState>('idle')
const updateInfo = ref<{ version: string; url: string; sha256: string } | null>(null)
const updateError = ref<string>('')

// Compares versions like "0.15.10-fork.4" by their numeric parts so we only
// ever offer a strictly-newer release (never a downgrade).
function isNewer(remote: string, local: string): boolean {
	const parse = (v: string) => (v.match(/\d+/g) || []).map(Number)
	const a = parse(remote)
	const b = parse(local)
	for (let i = 0; i < Math.max(a.length, b.length); i++) {
		const x = a[i] ?? 0
		const y = b[i] ?? 0
		if (x !== y) return x > y
	}
	return false
}

async function checkForUpdates() {
	updateState.value = 'checking'
	updateError.value = ''
	try {
		const res = await fetch(UPDATE_API, {
			headers: { Accept: 'application/json' },
		})
		if (!res.ok) throw new Error(`Update server returned ${res.status}`)
		const data = await res.json()
		const latest = String(data.tag_name || '').replace(/^v/, '')
		// The standalone app binary, never an installer.
		const asset = (data.assets || []).find(
			(a: GithubAsset) =>
				/\.exe$/i.test(a?.name ?? '') && !/setup|installer/i.test(a?.name ?? ''),
		)
		if (latest && asset?.browser_download_url && isNewer(latest, currentVersion.value)) {
			updateInfo.value = {
				version: latest,
				url: asset.browser_download_url,
				sha256: (asset.digest ?? '').replace(/^sha256:/i, ''),
			}
			updateState.value = 'available'
		} else {
			updateState.value = 'current'
		}
	} catch (e) {
		updateError.value = e instanceof Error ? e.message : String(e)
		updateState.value = 'error'
	}
}

async function installUpdate() {
	if (!updateInfo.value) return
	updateState.value = 'installing'
	updateError.value = ''
	try {
		await invoke('plugin:addons|fork_apply_update', {
			downloadUrl: updateInfo.value.url,
			expectedSha256: updateInfo.value.sha256 || null,
		})
		// The app restarts on success, so this normally does not return.
	} catch (e) {
		updateError.value = e instanceof Error ? e.message : String(e)
		updateState.value = 'error'
	}
}

const confirmingUninstall = ref<boolean>(false)
const uninstallError = ref<string>('')

async function uninstall() {
	uninstallError.value = ''
	try {
		await invoke('plugin:addons|fork_uninstall')
		// The app quits on success so the uninstaller can run.
	} catch (e) {
		uninstallError.value = e instanceof Error ? e.message : String(e)
		confirmingUninstall.value = false
	}
}
</script>

<template>
	<div class="flex flex-col gap-4 min-w-[600px]">
		<div class="flex items-center justify-between gap-4">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">App updates</h2>
				<p class="m-0 mt-1 text-sm">
					<span>You're on ByteLauncher v{{ currentVersion }} (based on Modrinth {{ currentVersion.split('-fork')[0] }}). </span>
					<span v-if="updateState === 'current'">You're up to date.</span>
					<span v-else-if="updateState === 'available'"
						>Update available: v{{ updateInfo?.version }}.</span
					>
					<span v-else-if="updateState === 'installing'"
						>Downloading and installing — the app will restart…</span
					>
					<span v-else-if="updateState === 'error'">Update failed: {{ updateError }}</span>
				</p>
			</div>
			<div class="flex items-center gap-2">
				<ButtonStyled v-if="updateState === 'available'" color="brand">
					<button :disabled="updateState === 'installing'" @click="installUpdate">
						Download &amp; install
					</button>
				</ButtonStyled>
				<ButtonStyled>
					<button
						:disabled="updateState === 'checking' || updateState === 'installing'"
						@click="checkForUpdates"
					>
						{{ updateState === 'checking' ? 'Checking…' : 'Check for updates' }}
					</button>
				</ButtonStyled>
			</div>
		</div>
		<div class="flex items-center justify-between gap-4">
			<div>
				<h2 class="m-0 text-lg font-semibold text-contrast">Plugins</h2>
				<p class="m-0 mt-1 text-sm">
					Toggle plugins on or off. Plugins live in your plugins folder — add your own by dropping
					in a folder containing a <code>manifest.json</code> with its <code>.js</code>/<code>.css</code>.
				</p>
			</div>
			<ButtonStyled>
				<button @click="openFolder">Open plugins folder</button>
			</ButtonStyled>
		</div>

		<div v-if="restartNeeded" class="flex items-center justify-between gap-4">
			<p class="m-0 text-sm">Restart the app to apply your changes.</p>
			<ButtonStyled color="brand">
				<button @click="restartApp">Restart now</button>
			</ButtonStyled>
		</div>

		<div v-if="plugins.length > 0" class="flex flex-wrap items-center gap-2">
			<label class="plugin-search">
				<SearchIcon class="plugin-search-icon" aria-hidden="true" />
				<input
					v-model="searchQuery"
					type="text"
					placeholder="Search plugins…"
					aria-label="Search plugins"
				/>
			</label>
			<div class="flex flex-wrap gap-1">
				<button
					v-for="f in PLUGIN_FILTERS"
					:key="f.id"
					class="chip"
					:class="{ active: filterMode === f.id }"
					@click="filterMode = f.id"
				>
					{{ f.label }}
				</button>
			</div>
			<div class="flex flex-wrap items-center gap-1">
				<span class="text-xs text-secondary">Sort:</span>
				<button
					v-for="s in PLUGIN_SORTS"
					:key="s.id"
					class="chip"
					:class="{ active: sortMode === s.id }"
					@click="sortMode = s.id"
				>
					{{ s.label }}
				</button>
			</div>
		</div>

		<div v-for="plugin in filteredPlugins" :key="plugin.id" class="flex flex-col gap-3">
			<div class="flex items-center justify-between gap-4">
				<div>
					<h2 class="m-0 text-lg font-semibold text-contrast">
						{{ plugin.name }}
						<span v-if="plugin.builtin" class="text-sm font-normal">(built-in)</span>
						<span v-if="isNewPlugin(plugin)" class="new-badge">New</span>
					</h2>
					<p class="m-0 mt-1 text-sm">{{ plugin.description }}</p>
					<p v-if="plugin.version || plugin.author" class="m-0 mt-1 text-sm">
						<span v-if="plugin.version">v{{ plugin.version }}</span>
						<span v-if="plugin.author"> · {{ plugin.author }}</span>
					</p>
				</div>
				<Toggle
					:id="`plugin-${plugin.id}`"
					:model-value="plugin.enabled"
					@update:model-value="() => toggle(plugin)"
				/>
			</div>
			<div
				v-if="plugin.id === 'hosting' && plugin.enabled"
				class="rounded-xl bg-bg p-3 border-[1px] border-solid border-surface-5"
			>
				<p class="m-0 mb-2 text-sm font-semibold text-contrast">Panel</p>
				<div class="flex flex-wrap gap-2">
					<button
						v-for="panel in HOSTING_PANELS"
						:key="panel.id"
						class="panel-option"
						:class="{ active: panel.id === selectedPanelId }"
						@click="setHostingPanel(panel.id)"
					>
						{{ panel.name }}
					</button>
				</div>
				<p class="m-0 mt-2 text-xs text-secondary">Loading: {{ selectedPanel.host }}</p>
			</div>
			<div
				v-if="plugin.id === 'hide-news' && plugin.enabled"
				class="rounded-xl bg-bg p-3 border-[1px] border-solid border-surface-5"
			>
				<div class="flex items-center justify-between gap-3">
					<div>
						<p class="m-0 text-sm font-semibold text-contrast">Hide the entire news section</p>
						<p class="m-0 mt-1 text-xs text-secondary">
							Removes the News heading, articles and “View all news” from the sidebar.
						</p>
					</div>
					<Toggle
						id="hide-all-news"
						:model-value="hideAllNews"
						@update:model-value="(v) => setHideAllNews(v)"
					/>
				</div>
			</div>
		</div>

		<p v-if="filteredPlugins.length === 0" class="m-0 text-sm text-secondary">
			{{ plugins.length === 0 ? 'No plugins found.' : 'No plugins match your filters.' }}
		</p>

		<div class="mt-2 border-0 border-t border-solid border-surface-5 pt-6">
			<h2 class="m-0 text-lg font-semibold text-red">Danger zone</h2>
			<div class="mt-3 flex items-center justify-between gap-4">
				<div>
					<h3 class="m-0 text-base font-semibold text-contrast">Uninstall ByteLauncher</h3>
					<p class="m-0 mt-1 text-sm">
						Reverts to the Modrinth App — restores <code>Modrinth App.exe</code>, removes
						ByteLauncher and closes. Your instances, accounts and settings are kept.
					</p>
					<p v-if="uninstallError" class="m-0 mt-1 text-sm text-red">{{ uninstallError }}</p>
				</div>
				<div class="flex shrink-0 items-center gap-2">
					<ButtonStyled v-if="!confirmingUninstall" color="red">
						<button @click="confirmingUninstall = true">Uninstall</button>
					</ButtonStyled>
					<template v-else>
						<ButtonStyled>
							<button @click="confirmingUninstall = false">Cancel</button>
						</ButtonStyled>
						<ButtonStyled color="red">
							<button @click="uninstall">Yes, revert to Modrinth</button>
						</ButtonStyled>
					</template>
				</div>
			</div>
		</div>
	</div>
</template>

<style scoped>
.panel-option {
	padding: 0.4rem 0.85rem;
	border-radius: var(--radius-md);
	border: 1px solid var(--color-button-bg);
	background: var(--color-button-bg);
	color: var(--color-base);
	font-weight: 600;
	font-size: 0.875rem;
	cursor: pointer;
	transition: all 0.1s ease-in-out;
}

.panel-option:hover {
	filter: brightness(0.85);
	color: var(--color-contrast);
}

.panel-option.active {
	background: var(--color-brand);
	border-color: var(--color-brand);
	color: #ffffff;
}

.plugin-search {
	position: relative;
	display: flex;
	align-items: center;
	flex: 1 1 200px;
	min-width: 180px;
}

.plugin-search-icon {
	position: absolute;
	left: 0.6rem;
	width: 1rem;
	height: 1rem;
	color: var(--color-secondary);
	pointer-events: none;
}

.plugin-search input {
	width: 100%;
	padding: 0.45rem 0.6rem 0.45rem 2rem;
	border-radius: var(--radius-md);
	border: 1px solid var(--color-button-bg);
	background: var(--color-raised-bg);
	color: var(--color-contrast);
	font-size: 0.875rem;
}

.plugin-search input::placeholder {
	color: var(--color-secondary);
}

.chip {
	padding: 0.4rem 0.75rem;
	border-radius: var(--radius-md);
	border: 1px solid var(--color-button-bg);
	background: var(--color-button-bg);
	color: var(--color-base);
	font-weight: 600;
	font-size: 0.8rem;
	cursor: pointer;
	transition: all 0.1s ease-in-out;
}

.chip:hover {
	color: var(--color-contrast);
}

.chip.active {
	background: var(--color-brand);
	border-color: var(--color-brand);
	color: #ffffff;
}

.new-badge {
	display: inline-block;
	margin-left: 0.4rem;
	padding: 0.05rem 0.4rem;
	border-radius: var(--radius-sm);
	background: var(--color-brand);
	color: #ffffff;
	font-size: 0.7rem;
	font-weight: 600;
	vertical-align: middle;
}
</style>
