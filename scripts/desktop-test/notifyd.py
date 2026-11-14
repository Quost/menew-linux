#!/usr/bin/env python3
"""Minimal org.freedesktop.Notifications server for headless sessions that have none (KWin only).

Logs every Notify call as 'notify: summary=<s> body=<b>' so the test can assert on it."""
import signal
from gi.repository import Gio, GLib

XML = """<node><interface name="org.freedesktop.Notifications">
<method name="GetCapabilities"><arg type="as" direction="out"/></method>
<method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/>
<arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/>
<arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
<method name="CloseNotification"><arg type="u" direction="in"/></method>
<method name="GetServerInformation"><arg type="s" direction="out"/><arg type="s" direction="out"/>
<arg type="s" direction="out"/><arg type="s" direction="out"/></method>
</interface></node>"""
counter = [0]


def handle(c, sender, path, iface, method, params, inv):
    if method == "GetCapabilities":
        inv.return_value(GLib.Variant("(as)", (["body"],)))
    elif method == "Notify":
        app, _rid, _icon, summary, body = params.unpack()[:5]
        counter[0] += 1
        print(f"notify: app={app} summary={summary} body={body}", flush=True)
        inv.return_value(GLib.Variant("(u)", (counter[0],)))
    elif method == "GetServerInformation":
        inv.return_value(GLib.Variant("(ssss)", ("menew-test-notifyd", "menew", "1", "1.2")))
    else:
        inv.return_value(None)


def on_bus(c, name):
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    c.register_object("/org/freedesktop/Notifications", node.interfaces[0], handle, None, None)


Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.Notifications", Gio.BusNameOwnerFlags.NONE,
                 on_bus, lambda c, n: print("notifyd ready", flush=True), None)
loop = GLib.MainLoop()
signal.signal(signal.SIGTERM, lambda *_: loop.quit())
loop.run()
