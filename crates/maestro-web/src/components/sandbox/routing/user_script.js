window.sendRuntimeMessage = async (message) => {
    return await chrome.runtime.sendMessage({
        ...message,
        sandboxId: __SANDBOX_ID__
    });
};
