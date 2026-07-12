import { invoke } from '@tauri-apps/api/core'

// Loads user/built-in plugins on startup. Plugins live in
// <app data>/plugins/<id>/ and are read by the Rust `addons` plugin. Each
// enabled plugin's CSS is injected as a <style> and its JS is executed.
//
// JS runs via indirect eval, which executes in global scope and is permitted by
// `script-src 'unsafe-eval'` in apps/app/tauri.conf.json. Unlike 'unsafe-inline'
// or a blob:/inline <script>, 'unsafe-eval' is NOT neutralized when Tauri adds a
// CSP nonce, so plugin JS runs reliably in WebView2.

const STYLE_ATTR = 'data-mr-plugin-style'

function injectCss(id, css) {
	const style = document.createElement('style')
	style.setAttribute(STYLE_ATTR, id)
	style.textContent = css
	document.head.appendChild(style)
}

function injectJs(id, js) {
	// Indirect eval: runs in global scope, allowed by script-src 'unsafe-eval'.
	const indirectEval = eval
	try {
		indirectEval(js)
	} catch (e) {
		console.error(`[plugins] error running "${id}"`, e)
	}
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
