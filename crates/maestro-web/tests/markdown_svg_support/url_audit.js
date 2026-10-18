const create = URL.createObjectURL.bind(URL);
const revoke = URL.revokeObjectURL.bind(URL);
globalThis.urlEvents = [];
globalThis.urlBlobs = new Map();
globalThis.urlIds = new Map();
URL.createObjectURL = blob => {
  const url = create(blob);
  const id = urlIds.size + 1;
  urlIds.set(url, id);
  urlBlobs.set(url, blob);
  urlEvents.push({op: 'create', id, type: blob.type, size: blob.size});
  return url;
};
URL.revokeObjectURL = url => {
  urlEvents.push({op: 'revoke', id: urlIds.get(url)});
  urlBlobs.delete(url);
  revoke(url);
};
