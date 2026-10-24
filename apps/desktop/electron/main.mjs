import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { app, BrowserWindow, ipcMain, net, protocol, session, shell } from "electron";
import { assetPath } from "./assets.mjs";
import { assertTrustedSender, authCallbackCode, externalUrl, validateInput } from "./security.mjs";
import { Sidecar } from "./sidecar.mjs";

protocol.registerSchemesAsPrivileged([
  {
    scheme: "oshwhub",
    privileges: {
      standard: true,
      secure: true,
      supportFetchAPI: true,
      corsEnabled: true,
    },
  },
]);

const here = path.dirname(fileURLToPath(import.meta.url));
const devUrl = !app.isPackaged && process.env.OSHWHUB_RENDERER_URL;
if (devUrl && devUrl !== "http://127.0.0.1:5173/")
  throw new Error("Unexpected development renderer URL");
const rendererUrl = devUrl || "oshwhub://app/";
const contentSecurityPolicy =
  "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' https: data: blob:; connect-src 'self'; font-src 'self' data:; worker-src 'self' blob:";
let mainWindow;
let core;
let rendererReady = false;
let pendingAuthCode;

function forwardAuthUrl(input) {
  const code = authCallbackCode(input);
  if (!code) return;
  if (!mainWindow || mainWindow.isDestroyed() || !rendererReady) {
    pendingAuthCode = code;
    return;
  }
  mainWindow.webContents.send("oshwhub:auth-callback", code);
}

app.on("open-url", (event, url) => {
  event.preventDefault();
  forwardAuthUrl(url);
});

const singleInstance = app.requestSingleInstanceLock();
if (!singleInstance) {
  app.quit();
} else {
  app.on("second-instance", (_event, commandLine) => {
    forwardAuthUrl(commandLine.at(-1));
    if (mainWindow && !mainWindow.isDestroyed()) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.focus();
    }
  });
  if (process.platform !== "darwin") forwardAuthUrl(process.argv.at(-1));
}

function createWindow() {
  mainWindow = new BrowserWindow({
    title: "OSHWHub · 开源硬件",
    width: 1440,
    height: 940,
    minWidth: 320,
    minHeight: 480,
    webPreferences: {
      preload: path.join(here, "preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      webviewTag: false,
    },
  });
  mainWindow.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  mainWindow.webContents.on("will-navigate", (event) => event.preventDefault());
  mainWindow.webContents.on("will-frame-navigate", (event) => event.preventDefault());
  mainWindow.webContents.on("will-attach-webview", (event) => event.preventDefault());
  mainWindow.on("closed", () => {
    mainWindow = undefined;
    rendererReady = false;
  });
  mainWindow.webContents.once("did-finish-load", () => {
    rendererReady = true;
    if (pendingAuthCode) {
      const code = pendingAuthCode;
      pendingAuthCode = undefined;
      mainWindow.webContents.send("oshwhub:auth-callback", code);
    }
  });
  mainWindow.loadURL(rendererUrl).catch(() => app.quit());
}

app
  .whenReady()
  .then(async () => {
    app.setAsDefaultProtocolClient("oshwhub");
    app.setAppLogsPath();
    const executable = `oshwhub-desktop-core${process.platform === "win32" ? ".exe" : ""}`;
    const binary = app.isPackaged
      ? path.join(process.resourcesPath, "core", executable)
      : path.join(here, "bin", `${process.platform}-${process.arch}`, executable);
    core = new Sidecar(
      spawn(binary, ["--log-dir", app.getPath("logs")], {
        stdio: ["pipe", "pipe", "pipe"],
        windowsHide: true,
      }),
    );
    session.defaultSession.setPermissionRequestHandler((_contents, _permission, callback) =>
      callback(false),
    );
    session.defaultSession.setPermissionCheckHandler(() => false);
    session.defaultSession.on("will-download", (event) => event.preventDefault());
    await protocol.handle("oshwhub", async (request) => {
      if (request.method !== "GET" && request.method !== "HEAD")
        return new Response(null, { status: 405 });
      try {
        const file = await assetPath(request.url, path.join(here, "..", "dist"));
        const response = await net.fetch(pathToFileURL(file).href);
        const headers = new Headers(response.headers);
        headers.set("Content-Security-Policy", contentSecurityPolicy);
        return new Response(request.method === "HEAD" ? null : response.body, {
          status: response.status,
          headers,
        });
      } catch {
        return new Response("Asset not found", { status: 404 });
      }
    });
    ipcMain.handle("oshwhub:dispatch", (event, input) => {
      assertTrustedSender(event, mainWindow, rendererUrl);
      return core.dispatch(validateInput(input));
    });
    ipcMain.handle("oshwhub:openExternal", async (event, url) => {
      assertTrustedSender(event, mainWindow, rendererUrl);
      await shell.openExternal(externalUrl(url));
    });
    createWindow();
    app.on("activate", () => {
      if (!mainWindow) createWindow();
    });
  })
  .catch(() => app.quit());

app.on("before-quit", () => core?.close());
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
