(function() {
	document.addEventListener('click', function(e) {
		const link = e.target.closest('a');
		if (link && link.hasAttribute('href')) {
			const address = document.createElement('a');
			address.href = link.getAttribute('href');
			if (address.protocol === 'http:' || address.protocol === 'https:') {
				e.preventDefault();
				e.stopPropagation();
				window.parent.postMessage({ type: 'open-external-url', url: address.href }, '*');
			}
		}
	}, true);

	document.addEventListener('submit', function(e) {
		const form = e.target;
		if (form && form.action) {
			e.preventDefault();
			e.stopPropagation();
			window.parent.postMessage({ type: 'open-external-url', url: form.action }, '*');
		}
	}, true);

	try {
		const originalLocation = window.location;
		Object.defineProperty(window, 'location', {
			get: function() { return originalLocation; },
			set: function(url) {
				window.parent.postMessage({ type: 'open-external-url', url: url.toString() }, '*');
			}
		});
	} catch (e) {
	}
})();