const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld(
  "oshwhubDesktop",
  Object.freeze({
    dispatch: (input) => ipcRenderer.invoke("oshwhub:dispatch", input),
    openExternal: (url) => ipcRenderer.invoke("oshwhub:openExternal", url),
  }),
);
