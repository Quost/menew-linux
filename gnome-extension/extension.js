import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import St from 'gi://St';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const MENEW_DBUS_NAME = 'org.menew.ClipboardService';
const MENEW_DBUS_PATH = '/org/menew/Clipboard';
const MENEW_DBUS_IFACE = 'org.menew.ClipboardService';

const ClipboardIfaceXml = `
<node>
  <interface name="${MENEW_DBUS_IFACE}">
    <method name="PushClipboardText">
      <arg type="s" name="text" direction="in"/>
      <arg type="s" name="mime" direction="in"/>
      <arg type="b" name="success" direction="out"/>
    </method>
    <method name="Ping">
      <arg type="s" name="reply" direction="out"/>
    </method>
  </interface>
</node>`;

export default class MenewClipboardExtension extends Extension {
    enable() {
        console.log('[MenewExtension] Enabling GNOME Wayland clipboard bridge...');
        this._selectionHandlerId = 0;
        this._lastCopiedText = null;
        this._dbusProxy = null;

        this._initDbusProxy();
        this._startClipboardWatcher();
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

    _onClipboardChanged(source) {
        // Privacy check: verify available MIME types for password manager hints
        if (source && typeof source.get_mimetypes === 'function') {
            const mimes = source.get_mimetypes();
            const hasSecretHint = mimes.some(m => {
                const lower = m.toLowerCase();
                return (
                    lower.includes('passwordmanagerhint') ||
                    lower.includes('concealedtype') ||
                    lower === 'x-kde-passwordmanagerhint' ||
                    lower === 'application/x-kde-passwordmanagerhint'
                );
            });

            if (hasSecretHint) {
                console.log('[MenewExtension] Ignoring clipboard content marked secret by password manager');
                return;
            }
        }

        const clipboard = St.Clipboard.get_default();
        clipboard.get_text(St.ClipboardType.CLIPBOARD, (clip, text) => {
            if (!text || text.trim().length === 0) {
                return;
            }

            if (text === this._lastCopiedText) {
                return;
            }

            this._lastCopiedText = text;
            this._sendToMenew(text);
        });
    }

    _sendToMenew(text) {
        if (!this._dbusProxy) {
            this._initDbusProxy();
        }

        if (this._dbusProxy) {
            this._dbusProxy.PushClipboardTextRemote(text, 'text/plain', (result, error) => {
                if (error) {
                    console.debug(`[MenewExtension] Failed sending text to Menew daemon: ${error.message}`);
                }
            });
        }
    }
}
