(_sandboxId) => {
            window.returnDownloadableFile = async (fileName, content, mimeType) => {
                let finalContent, finalMimeType;
                if (content instanceof Blob) {
                    const arrayBuffer = await content.arrayBuffer();
                    finalContent = new Uint8Array(arrayBuffer);
                    finalMimeType = mimeType || content.type || "application/octet-stream";
                    if (!mimeType && !content.type) {
                        throw new Error("returnDownloadableFile: MIME type is required for Blob content. Please provide a mimeType parameter (e.g., 'image/png').");
                    }
                }
                else if (content instanceof Uint8Array) {
                    finalContent = content;
                    if (!mimeType) {
                        throw new Error("returnDownloadableFile: MIME type is required for Uint8Array content. Please provide a mimeType parameter (e.g., 'image/png').");
                    }
                    finalMimeType = mimeType;
                }
                else if (typeof content === "string") {
                    finalContent = content;
                    finalMimeType = mimeType || "text/plain";
                }
                else {
                    finalContent = JSON.stringify(content, null, 2);
                    finalMimeType = mimeType || "application/json";
                }
                // Send to extension if in extension context (online mode)
                if (window.sendRuntimeMessage) {
                    const response = await window.sendRuntimeMessage({
                        type: "file-returned",
                        fileName,
                        content: finalContent,
                        mimeType: finalMimeType,
                    });
                    if (response.error)
                        throw new Error(response.error);
                }
                else {
                    // Offline mode: trigger browser download directly
                    const blob = new Blob([finalContent instanceof Uint8Array ? finalContent : finalContent], {
                        type: finalMimeType,
                    });
                    const url = URL.createObjectURL(blob);
                    const a = document.createElement("a");
                    a.href = url;
                    a.download = fileName;
                    a.click();
                    URL.revokeObjectURL(url);
                }
            };
        }