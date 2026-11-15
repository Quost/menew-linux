import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import St from 'gi://St';
import Shell from 'gi://Shell';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const MENEW_DBUS_NAME = 'org.menew.ClipboardService';
const MENEW_DBUS_PATH = '/org/menew/Clipboard';
const MENEW_DBUS_IFACE = 'org.menew.ClipboardService';

// The daemon refuses more than this too; bigger copies are not even sent over D-Bus.
const MAX_BYTES = 20 * 1024 * 1024;

const TEXT_TYPES = ['text/plain;charset=utf-8', 'text/plain', 'UTF8_STRING', 'STRING', 'TEXT'];
const IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/webp', 'image/gif', 'image/bmp', 'image/tiff'];

const ClipboardIfaceXml = `
<node>
  <interface name="${MENEW_DBUS_IFACE}">
    <method name="PushClipboardText">
      <arg type="s" name="text" direction="in"/>
      <arg type="s" name="mime" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="PushClipboardData">
      <arg type="s" name="mime" direction="in"/>
      <arg type="ay" name="data" direction="in"/>
      <arg type="s" name="app_id" direction="in"/>
      <arg type="s" name="app_name" direction="in"/>
      <arg type="as" name="types" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="ShouldCapture">
      <arg type="s" name="app_id" direction="in"/>
      <arg type="s" name="app_name" direction="in"/>
      <arg type="b" name="allowed" direction="out"/>
    </method>
    <method name="Ping">
      <arg type="s" name="reply" direction="out"/>
    </method>
  </interface>
</node>`;

// Window snapping and the front app, called by the daemon (Wayland apps can't move other windows).
const ShellIfaceXml = `
<node>
  <interface name="org.menew.Shell">
    <method name="SnapFocused">
      <arg type="d" name="x" direction="in"/>
      <arg type="d" name="y" direction="in"/>
      <arg type="d" name="width" direction="in"/>
      <arg type="d" name="height" direction="in"/>
      <arg type="b" name="done" direction="out"/>
    </method>
    <method name="MaximizeFocused">
      <arg type="b" name="done" direction="out"/>
    </method>
    <method name="CenterFocused">
      <arg type="b" name="done" direction="out"/>
    </method>
    <method name="MoveFocusedToNextMonitor">
      <arg type="b" name="done" direction="out"/>
    </method>
    <method name="FocusedApp">
      <arg type="s" name="app_id" direction="out"/>
      <arg type="s" name="app_name" direction="out"/>
    </method>
  </interface>
</node>`;

/** The window people were using: Menew's own palette never counts. */
function frontWindow() {
    const win = global.display.focus_window;
    if (win && win.get_gtk_application_id?.() === 'app.menew.Menew') {
        // the palette is closing: take the next window of the stack
        const list = global.display.get_tab_list(Meta.TabList.NORMAL, null);
        return list.find(w => w !== win) || null;
    }
    if (!win || win.get_window_type() !== Meta.WindowType.NORMAL)
        return null;
    return win;
}

function unmaximize(win) {
    try {
        win.unmaximize(Meta.MaximizeFlags.BOTH);
    } catch (e) {
        // GNOME 49 dropped the flags argument
        try { win.unmaximize(); } catch (e2) { /* not maximized */ }
    }
}

class ShellService {
    SnapFocused(x, y, width, height) {
        const win = frontWindow();
        if (!win)
            return false;
        unmaximize(win);
        const a = win.get_work_area_current_monitor();
        win.move_resize_frame(true, Math.round(a.x + a.width * x), Math.round(a.y + a.height * y),
            Math.round(a.width * width), Math.round(a.height * height));
        win.activate(global.get_current_time());
        return true;
    }

    MaximizeFocused() {
        const win = frontWindow();
        if (!win)
            return false;
        try {
            win.maximize(Meta.MaximizeFlags.BOTH);
        } catch (e) {
            win.maximize();
        }
        return true;
    }

    CenterFocused() {
        const win = frontWindow();
        if (!win)
            return false;
        const a = win.get_work_area_current_monitor();
        const r = win.get_frame_rect();
        win.move_frame(true, Math.round(a.x + (a.width - r.width) / 2), Math.round(a.y + (a.height - r.height) / 2));
        return true;
    }

    MoveFocusedToNextMonitor() {
        const win = frontWindow();
        const n = global.display.get_n_monitors();
        if (!win || n < 2)
            return false;
        win.move_to_monitor((win.get_monitor() + 1) % n);
        return true;
    }

    FocusedApp() {
        const win = frontWindow();
        if (!win)
            return ['', ''];
        const id = win.get_gtk_application_id?.() || win.get_wm_class?.() || '';
        let name = id;
        try {
            const app = Shell.WindowTracker.get_default().get_window_app(win);
            if (app)
                name = app.get_name();
        } catch (e) { /* the class is enough */ }
        return [id, name];
    }
}

function isSecretType(mime) {
    const m = mime.toLowerCase();
    return m.includes('password') || m.includes('secret') || m.includes('concealedtype') ||
        m.includes('transienttype') || m.includes('autogeneratedtype') || m.includes('onepassword');
}

export default class MenewClipboardExtension extends Extension {
    enable() {
        console.log('[MenewExtension] Enabling GNOME Wayland clipboard bridge...');
        this._selectionHandlerId = 0;
        this._lastCopiedText = null;
        this._dbusProxy = null;

        this._initDbusProxy();
        this._startClipboardWatcher();
        this._exportShellService();
    }

    _exportShellService() {
        try {
            this._shellService = Gio.DBusExportedObject.wrapJSObject(ShellIfaceXml, new ShellService());
            this._shellService.export(Gio.DBus.session, '/org/menew/Shell');
            this._shellNameId = Gio.bus_own_name_on_connection(Gio.DBus.session, 'org.menew.Shell',
                Gio.BusNameOwnerFlags.NONE, null, null);
            console.log('[MenewExtension] Window service exported as org.menew.Shell');
        } catch (e) {
            console.error(`[MenewExtension] Could not export org.menew.Shell: ${e.message}`);
        }
    }

    disable() {
        console.log('[MenewExtension] Disabling GNOME Wayland clipboard bridge...');
        if (this._selectionHandlerId && global.display) {
            const selection = global.display.get_selection();
            if (selection) {
                selection.disconnect(this._selectionHandlerId);
            }
            this._selectionHandlerId = 0;
        }

        this._dbusProxy = null;
        this._lastCopiedText = null;
        if (this._shellNameId) {
            Gio.bus_unown_name(this._shellNameId);
            this._shellNameId = 0;
        }
        if (this._shellService) {
            this._shellService.unexport();
            this._shellService = null;
        }
    }

    _initDbusProxy() {
        try {
            const ProxyClass = Gio.DBusProxy.makeProxyWrapper(ClipboardIfaceXml);
            this._dbusProxy = new ProxyClass(
                Gio.DBus.session,
                MENEW_DBUS_NAME,
                MENEW_DBUS_PATH,
                (proxy, error) => {
                    if (error) {
                        console.warn(`[MenewExtension] Could not connect to Menew D-Bus service: ${error.message}`);
                    } else {
                        console.log('[MenewExtension] Connected to Menew D-Bus service successfully.');
                    }
                }
            );
        } catch (e) {
            console.error(`[MenewExtension] Error creating D-Bus proxy: ${e.message}`);
        }
    }

    _startClipboardWatcher() {
        try {
            const selection = global.display.get_selection();
            if (!selection) {
                console.warn('[MenewExtension] global.display.get_selection() returned null');
                return;
            }

            this._selectionHandlerId = selection.connect('owner-changed', (sel, selectionType, source) => {
                if (selectionType === Meta.SelectionType.SELECTION_CLIPBOARD) {
                    this._onClipboardChanged(source);
                }
            });
            console.log('[MenewExtension] Subscribed to Meta.Selection owner-changed');
        } catch (e) {
            console.error(`[MenewExtension] Failed to connect to selection owner-changed: ${e.message}`);
        }
    }

    /** The app that has the focus now, which is the one that copied. */
    _focusedApp() {
        try {
            const win = global.display.focus_window;
            if (!win)
                return ['', ''];
            const wmClass = win.get_gtk_application_id?.() || win.get_wm_class?.() || '';
            let name = '';
            try {
                const app = Shell.WindowTracker.get_default().get_window_app(win);
                name = app ? app.get_name() : '';
            } catch (e) {
                // no tracker: the class is enough
            }
            return [wmClass, name || wmClass];
        } catch (e) {
            return ['', ''];
        }
    }

    _onClipboardChanged(source) {
        const mimes = (source && typeof source.get_mimetypes === 'function') ? source.get_mimetypes() : [];

        // Privacy check: password manager hints, for text, images and files alike
        if (mimes.some(isSecretType)) {
            console.log('[MenewExtension] Ignoring clipboard content marked secret by password manager');
            return;
        }

        if (!this._dbusProxy) {
            this._initDbusProxy();
        }
        if (!this._dbusProxy)
            return;

        const [appId, appName] = this._focusedApp();
        // Ask first: while capture is paused (or the app is ignored) the clipboard is not read at all
        this._dbusProxy.ShouldCaptureRemote(appId, appName, (result, error) => {
            if (error) {
                console.debug(`[MenewExtension] ShouldCapture failed: ${error.message}`);
                return;
            }
            if (!result || result[0] !== true) {
                console.log('[MenewExtension] Capture paused or app ignored: not reading the clipboard');
                return;
            }
            this._readAndSend(mimes, appId, appName);
        });
    }

    _readAndSend(mimes, appId, appName) {
        const clipboard = St.Clipboard.get_default();
        console.log(`[MenewExtension] Copy offers: ${mimes.join(', ')}`);
        const lower = mimes.map(m => m.toLowerCase());
        const hasFiles = lower.includes('text/uri-list');
        const imageType = IMAGE_TYPES.find(t => lower.includes(t));
        const hasText = TEXT_TYPES.some(t => lower.includes(t.toLowerCase())) || mimes.length === 0;

        const sendImage = () => {
            if (!imageType)
                return;
            clipboard.get_content(St.ClipboardType.CLIPBOARD, imageType, (clip, bytes) => {
                if (bytes && bytes.get_size() > 0)
                    this._send(imageType, bytes, appId, appName, mimes);
            });
        };
        const sendText = () => {
            if (!hasText) {
                sendImage();
                return;
            }
            clipboard.get_text(St.ClipboardType.CLIPBOARD, (clip, text) => {
                if (!text || text.trim().length === 0) {
                    sendImage();
                    return;
                }
                if (text === this._lastCopiedText)
                    return;
                this._lastCopiedText = text;
                this._send('text/plain', new TextEncoder().encode(text), appId, appName, mimes);
            });
        };

        // like the Mac: files first, then text, then an image
        if (hasFiles) {
            clipboard.get_content(St.ClipboardType.CLIPBOARD, 'text/uri-list', (clip, bytes) => {
                const raw = bytes ? new TextDecoder().decode(bytes.toArray()) : '';
                console.log(`[MenewExtension] uri-list read: ${raw.length} characters`);
                if (raw.includes('file://'))
                    this._send('text/uri-list', bytes, appId, appName, mimes);
                else
                    sendText();
            });
        } else {
            sendText();
        }
    }

    _send(mime, data, appId, appName, types) {
        const bytes = data.toArray ? data.toArray() : data;
        if (bytes.length > MAX_BYTES) {
            console.log(`[MenewExtension] Copy of ${bytes.length} bytes is over the limit, not sent`);
            return;
        }
        this._dbusProxy.PushClipboardDataRemote(mime, bytes, appId, appName, types, (result, error) => {
            if (error) {
                console.debug(`[MenewExtension] Failed sending ${mime} to Menew daemon: ${error.message}`);
            }
        });
    }
}
