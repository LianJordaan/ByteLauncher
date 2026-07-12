// Multi-Launch: adds a "Launch again" button right next to an instance's
// Play/Stop button so you can run the same instance multiple times at once. The
// Rust core has no dedupe guard (each launch gets a fresh process UUID), so this
// simply calls the same command the normal Play button uses.
;(function () {
	const BUTTON_ID = 'mr-multi-launch-btn'
	// The primary action button's text is hardcoded (not translated) to one of:
	const PRIMARY_LABELS = ['play', 'stop', 'stopping...', 'starting...']

	function currentInstanceId() {
		const match = window.location.pathname.match(/\/instance\/([^/]+)/)
		return match ? decodeURIComponent(match[1]) : null
	}

	async function launchAgain() {
		const instanceId = currentInstanceId()
		if (!instanceId) return
		const invoke = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke
		if (!invoke) {
			console.error('[multi-launch] Tauri invoke is unavailable')
			return
		}
		try {
			await invoke('plugin:instance|instance_run', { instanceId, serverAddress: null })
		} catch (e) {
			console.error('[multi-launch] failed to launch instance', e)
		}
	}

	function findPrimaryButton() {
		for (const button of document.querySelectorAll('button')) {
			const text = (button.textContent || '').trim().toLowerCase()
			if (PRIMARY_LABELS.includes(text)) return button
		}
		return null
	}

	// Walk up from the Play/Stop button to the direct child of the
	// `flex gap-2` action-bar row, which is where we insert our button.
	function actionBarChild(primary) {
		let el = primary
		while (el && el.parentElement) {
			const parent = el.parentElement
			if (parent.classList.contains('flex') && parent.classList.contains('gap-2')) {
				return el
			}
			el = parent
		}
		return null
	}

	function makeButton() {
		const button = document.createElement('button')
		button.id = BUTTON_ID
		button.type = 'button'
		button.textContent = '↻ Launch again'
		button.style.cssText = [
			'display:inline-flex',
			'align-items:center',
			'justify-content:center',
			'height:40px',
			'padding:0 16px',
			'border:none',
			'border-radius:12px',
			'cursor:pointer',
			'font-weight:600',
			'font-size:1rem',
			'white-space:nowrap',
			'background:var(--color-brand, #1bd96a)',
			'color:#000',
		].join(';')
		button.addEventListener('click', (event) => {
			event.preventDefault()
			event.stopPropagation()
			launchAgain()
		})
		return button
	}

	function sync() {
		const existing = document.getElementById(BUTTON_ID)
		if (!currentInstanceId()) {
			if (existing) existing.remove()
			return
		}
		const primary = findPrimaryButton()
		const anchor = primary ? actionBarChild(primary) : null
		if (!anchor) {
			if (existing) existing.remove()
			return
		}
		const button = existing || makeButton()
		if (anchor.nextElementSibling !== button) {
			anchor.insertAdjacentElement('afterend', button)
		}
	}

	setInterval(sync, 400)
	sync()
})()
