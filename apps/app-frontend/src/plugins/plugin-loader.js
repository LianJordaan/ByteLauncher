import { invoke } from '@tauri-apps/api/core'

// Loads user/built-in plugins on startup. Plugins live in
// <app data>/plugins/<id>/ and are read by the Rust `addons` plugin. Each
// enabled plugin's CSS is injected as a <style> and its JS as a <script>.
//
// JS is injected via a blob: URL rather than an inline <script>, so it loads
// even when Tauri locks inline scripts with a CSP nonce/hash. `script-src` in
// apps/app/tauri.conf.json includes `blob:` for exactly this reason.

const STYLE_ATTR = 'data-mr-plugin-style'
const SCRIPT_ATTR = 'data-mr-plugin-script'

function injectCss(id, css) {
	const style = document.createElement('style')
	style.setAttribute(STYLE_ATTR, id)
	style.textContent = css
	document.head.appendChild(style)
}

function injectJs(id, js) {
	const blob = new Blob([js], { type: 'text/javascript' })
	const url = URL.createObjectURL(blob)
	const script = document.createElement('script')
	script.setAttribute(SCRIPT_ATTR, id)
	script.src = url
	script.addEventListener('load', () => URL.revokeObjectURL(url))
	script.addEventListener('error', (e) => console.error(`[plugins] error running ${id}`, e))
	document.head.appendChild(script)
}

export async function loadPlugins() {
	let plugins
	try {
		plugins = await invoke('plugin:addons|read_plugins')
	} catch (e) {
		console.error('[plugins] failed to read plugins', e)
		return
	}

	if (!Array.isArray(plugins)) return

	for (const plugin of plugins) {
		if (!plugin.enabled) continue
		try {
			if (plugin.css) injectCss(plugin.id, plugin.css)
			if (plugin.js) injectJs(plugin.id, plugin.js)
			console.log(`[plugins] loaded "${plugin.id}"`)
		} catch (e) {
			console.error(`[plugins] failed to load "${plugin.id}"`, e)
		}
	}
}
