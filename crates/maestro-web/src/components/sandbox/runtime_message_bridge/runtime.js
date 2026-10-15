window.__completionCallbacks = [];
__SEND_RUNTIME_MESSAGE__
window.onCompleted = (callback) => {
    window.__completionCallbacks.push(callback);
};
