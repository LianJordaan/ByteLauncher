// Multi-Launch: adds a floating "Launch again" button on instance pages that
// starts another copy of the current instance. The Rust core has no dedupe
// guard (each launch gets a fresh process UUID), so this simply calls the same
// command the normal Play button uses.
;(function () {
	const BUTTON_ID = 'mr-multi-launch-btn'

	function currentInstanceId() {
		const match = window.location.pathname.match(/\/instance\/([^/]+)/)
		return match ? decodeURIComponent(match[1]) : null
	}

	async function launchAgain(instanceId) {
		if (!instanceId) return
		const invoke = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke
		if (!invoke) {
			console.error('[multi-launch] Tauri invoke is unavailable')
			return
		}
		try {
			await invoke('plugin:instance|instance_run', {
				instanceId,
				serverAddress: null,
			})
		} catch (e) {
			console.error('[multi-launch] failed to launch instance', e)
		}
	}

	function syncButton() {
		const instanceId = currentInstanceId()
		let button = document.getElementById(BUTTON_ID)

		if (!instanceId) {
			if (button) button.remove()
			return
		}
		if (button) return

		button = document.createElement('button')
		button.id = BUTTON_ID
		button.textContent = '↻ Launch again'
		button.style.cssText = [
			'position:fixed',
			'right:16px',
			'bottom:16px',
			'z-index:9999',
			'padding:8px 14px',
			'border:none',
			'border-radius:8px',
			'cursor:pointer',
			'font-weight:600',
			'font-size:13px',
			'background:var(--color-brand, #1bd96a)',
			'color:#000',
			'box-shadow:0 2px 10px rgba(0,0,0,0.45)',
		].join(';')
		button.addEventListener('click', () => launchAgain(currentInstanceId()))
		document.body.appendChild(button)
	}

	setInterval(syncButton, 700)
	syncButton()
})()
