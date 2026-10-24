window.sendRuntimeMessage = async (message) => {
    const messageId = 'msg_' + Date.now() + '_' + Math.random().toString(36).substring(2, 9);

    return new Promise((resolve, reject) => {
        const handler = (e) => {
            if (e.data.type === 'runtime-response' && e.data.messageId === messageId) {
                window.removeEventListener('message', handler);
                if (e.data.success) {
                    resolve(e.data);
                } else {
                    reject(new Error(e.data.error || 'Operation failed'));
                }
            }
        };

        window.addEventListener('message', handler);

        window.parent.postMessage({
            ...message,
            sandboxId: __SANDBOX_ID__,
            messageId: messageId
        }, '*');

        // Timeout after 30s
        setTimeout(() => {
            window.removeEventListener('message', handler);
            reject(new Error('Runtime message timeout'));
        }, 30000);
    });
};
