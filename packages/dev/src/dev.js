(() => {
  const FILE = "skene-state";
  const root = () => navigator.storage.getDirectory();
  const encode = (bytes) => {
    let text = "";
    for (let at = 0; at < bytes.length; at += 8192) text += String.fromCharCode(...bytes.subarray(at, at + 8192));
    return btoa(text);
  };
  window.skene_state = async () => {
    try {
      const directory = await root();
      const file = await (await directory.getFileHandle(FILE)).getFile();
      const bytes = new Uint8Array(await file.arrayBuffer());
      await directory.removeEntry(FILE);
      return bytes;
    } catch {}
    try {
      const text = sessionStorage.getItem(FILE);
      if (text) {
        sessionStorage.removeItem(FILE);
        return Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
      }
    } catch {}
    return null;
  };
  const save = async () => {
    if (!window.skene_snapshot) return;
    const handle = await (await root()).getFileHandle(FILE, { create: true });
    const writable = await handle.createWritable();
    await writable.write(window.skene_snapshot());
    await writable.close();
  };
  addEventListener("pagehide", () => {
    if (!window.skene_snapshot) return;
    try { sessionStorage.setItem(FILE, encode(window.skene_snapshot())); } catch {}
  });
  new EventSource("/skene:reload").onmessage = () => {
    save().catch(() => {}).finally(() => location.reload());
  };
})();
