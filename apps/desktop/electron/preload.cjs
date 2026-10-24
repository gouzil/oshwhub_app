const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld(
  "oshwhubDesktop",
  Object.freeze({
    dispatch: (input) => ipcRenderer.invoke("oshwhub:dispatch", input),
    openExternal: (url) => ipcRenderer.invoke("oshwhub:openExternal", url),
    onAuthCallback: (listener) => {
      const receive = (_event, code) => listener(code);
      ipcRenderer.on("oshwhub:auth-callback", receive);
      return () => ipcRenderer.removeListener("oshwhub:auth-callback", receive);
    },
  }),
);
