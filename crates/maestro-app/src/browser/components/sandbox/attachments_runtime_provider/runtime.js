(_sandboxId) => {
            window.listAttachments = () => (window.attachments || []).map((a) => ({
                id: a.id,
                fileName: a.fileName,
                mimeType: a.mimeType,
                size: a.size,
            }));
            window.readTextAttachment = (attachmentId) => {
                const a = (window.attachments || []).find((x) => x.id === attachmentId);
                if (!a)
                    throw new Error(`Attachment not found: ${attachmentId}`);
                if (a.extractedText)
                    return a.extractedText;
                try {
                    return atob(a.content);
                }
                catch {
                    throw new Error(`Failed to decode text content for: ${attachmentId}`);
                }
            };
            window.readBinaryAttachment = (attachmentId) => {
                const a = (window.attachments || []).find((x) => x.id === attachmentId);
                if (!a)
                    throw new Error(`Attachment not found: ${attachmentId}`);
                const bin = atob(a.content);
                const bytes = new Uint8Array(bin.length);
                for (let i = 0; i < bin.length; i++)
                    bytes[i] = bin.charCodeAt(i);
                return bytes;
            };
        }