
				};

				const returnValue = await userCodeFunc();

				if (window.__completionCallbacks && window.__completionCallbacks.length > 0) {
					try {
						await Promise.all(window.__completionCallbacks.map(cb => cb(true)));
					} catch (e) {
						console.error('Completion callback error:', e);
					}
				}

				await window.complete(null, returnValue);
			} catch (error) {

				if (window.__completionCallbacks && window.__completionCallbacks.length > 0) {
					try {
						await Promise.all(window.__completionCallbacks.map(cb => cb(false)));
					} catch (e) {
						console.error('Completion callback error:', e);
					}
				}

				await window.complete({
					message: error?.message || String(error),
					stack: error?.stack || new Error().stack
				});
			}
		})();
