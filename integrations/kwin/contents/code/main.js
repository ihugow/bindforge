// Plasma 6 focus bridge. Loaded temporarily by the daemon; never remaps input.
// No window titles, executable paths, keystrokes, or app IDs in logs.
function publishFocus(window) {
    var desktopFile = window ? String(window.desktopFileName || "") : "";
    var resourceClass = window ? String(window.resourceClass || "") : "";
    var pid = window ? Number(window.pid) : 0;
    var processId = isFinite(pid) && pid > 0 && Math.floor(pid) === pid && pid <= 4294967295 ? String(pid) : "0";
    callDBus("org.bindforge.Daemon", "/org/bindforge/Daemon",
        "org.bindforge.Daemon.Focus1", "FocusChangedWithProcess", desktopFile, resourceClass, processId);
}
workspace.windowActivated.connect(publishFocus);
publishFocus(workspace.activeWindow);
