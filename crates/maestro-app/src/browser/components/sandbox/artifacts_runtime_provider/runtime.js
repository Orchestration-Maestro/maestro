(_sandboxId) => {
            // JSON artifacts are parsed on read
            const isJsonFile = (filename) => filename.endsWith(".json");
            window.listArtifacts = async () => {
                // Online: ask extension
                if (window.sendRuntimeMessage) {
                    const response = await window.sendRuntimeMessage({
                        type: "artifact-operation",
                        action: "list",
                    });
                    if (!response.success)
                        throw new Error(response.error);
                    return response.result;
                }
                // Offline: return snapshot keys
                else {
                    return Object.keys(window.artifacts || {});
                }
            };
            window.getArtifact = async (filename) => {
                let content;
                // Online: ask extension
                if (window.sendRuntimeMessage) {
                    const response = await window.sendRuntimeMessage({
                        type: "artifact-operation",
                        action: "get",
                        filename,
                    });
                    if (!response.success)
                        throw new Error(response.error);
                    content = response.result;
                }
                // Offline: read snapshot
                else {
                    if (!Object.hasOwn(window.artifacts || {}, filename)) {
                        throw new Error(`Artifact not found (offline mode): ${filename}`);
                    }
                    content = window.artifacts[filename];
                }
                // Auto-parse .json files
                if (isJsonFile(filename)) {
                    try {
                        return JSON.parse(content);
                    }
                    catch (e) {
                        throw new Error(`Failed to parse JSON from ${filename}: ${e}`);
                    }
                }
                return content;
            };
            window.createOrUpdateArtifact = async (filename, content, mimeType) => {
                if (!window.sendRuntimeMessage) {
                    throw new Error("Cannot create/update artifacts in offline mode (read-only)");
                }
                let finalContent = content;
                // Serialize non-string content once
                if (typeof content !== "string") {
                    finalContent = JSON.stringify(content, null, 2);
                }
                const response = await window.sendRuntimeMessage({
                    type: "artifact-operation",
                    action: "createOrUpdate",
                    filename,
                    content: finalContent,
                    mimeType,
                });
                if (!response.success)
                    throw new Error(response.error);
            };
            window.deleteArtifact = async (filename) => {
                if (!window.sendRuntimeMessage) {
                    throw new Error("Cannot delete artifacts in offline mode (read-only)");
                }
                const response = await window.sendRuntimeMessage({
                    type: "artifact-operation",
                    action: "delete",
                    filename,
                });
                if (!response.success)
                    throw new Error(response.error);
            };
        }