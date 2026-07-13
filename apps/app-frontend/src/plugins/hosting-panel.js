import { computed, ref } from 'vue'

// State for the built-in "ByteBuilders Hosting" plugin. The chosen panel is
// stored in localStorage (never the shared app.db). Every panel is loaded
// through a header-stripping proxy so it can be embedded in an <iframe> — the
// panels themselves send X-Frame-Options/frame-ancestors that would otherwise
// block framing.

const STORAGE_KEY = 'bytelauncher-hosting-panel'
const DEFAULT_PANEL_ID = 'main'

// Each panel is served at the ROOT of its own proxy subdomain
// (<id>.proxy.bytebuilders.co.za). Root-hosting — not a `/<host>` path prefix —
// is required so the panel's single-page-app routing resolves; a path prefix
// makes the SPA 404 when the tab is reopened at `/<host>`.
export const HOSTING_PANELS = [
	{ id: 'main', name: 'Main', host: 'panel.bytebuilders.co.za', url: 'https://main.proxy.bytebuilders.co.za/' },
	{ id: 'supers', name: 'SuperS Network', host: 'panel.supersnetwork.com', url: 'https://supers.proxy.bytebuilders.co.za/' },
	{ id: 'kia', name: 'Kia', host: 'kia.bytebuilders.co.za', url: 'https://kia.proxy.bytebuilders.co.za/' },
]

function loadPanelId() {
	const saved = localStorage.getItem(STORAGE_KEY)
	if (saved && HOSTING_PANELS.some((p) => p.id === saved)) return saved
	return DEFAULT_PANEL_ID
}

export const selectedPanelId = ref(loadPanelId())

export const selectedPanel = computed(
	() => HOSTING_PANELS.find((p) => p.id === selectedPanelId.value) ?? HOSTING_PANELS[0],
)

// The proxy-subdomain root URL to embed, e.g. https://main.proxy.bytebuilders.co.za/
export const hostingPanelUrl = computed(() => selectedPanel.value.url)

export function setHostingPanel(id) {
	if (!HOSTING_PANELS.some((p) => p.id === id)) return
	selectedPanelId.value = id
	try {
		localStorage.setItem(STORAGE_KEY, id)
	} catch (e) {
		console.error('[hosting] failed to save selected panel', e)
	}
}
