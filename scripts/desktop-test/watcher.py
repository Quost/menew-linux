#!/usr/bin/env python3
"""Minimal org.kde.StatusNotifierWatcher for headless sessions without plasmashell.

Implements just what ksni needs: RegisterStatusNotifierItem, RegisteredStatusNotifierItems,
IsStatusNotifierHostRegistered. Items are kept until they vanish from the bus."""
import signal
from gi.repository import Gio, GLib

XML = """<node><interface name="org.kde.StatusNotifierWatcher">
<method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
<method name="RegisterStatusNotifierHost"><arg type="s" direction="in"/></method>
<property name="RegisteredStatusNotifierItems" type="as" access="read"/>
<property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
<property name="ProtocolVersion" type="i" access="read"/>
<signal name="StatusNotifierItemRegistered"><arg type="s"/></signal>
<signal name="StatusNotifierHostRegistered"/>
</interface></node>"""
items = []
conn = None


def handle(c, sender, path, iface, method, params, inv):
    if method == "RegisterStatusNotifierItem":
        svc = params.unpack()[0]
        entry = f"{sender}{svc}" if svc.startswith("/") else svc
        if entry not in items:
            items.append(entry)
        c.emit_signal(None, path, iface, "StatusNotifierItemRegistered", GLib.Variant("(s)", (entry,)))
        print("registered:", entry, flush=True)
    inv.return_value(None)


def get_prop(c, sender, path, iface, name):
    if name == "RegisteredStatusNotifierItems":
        return GLib.Variant("as", items)
    if name == "IsStatusNotifierHostRegistered":
        return GLib.Variant("b", True)
    return GLib.Variant("i", 0)


def on_bus(c, name):
    global conn
    conn = c
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    c.register_object("/StatusNotifierWatcher", node.interfaces[0], handle, get_prop, None)


Gio.bus_own_name(Gio.BusType.SESSION, "org.kde.StatusNotifierWatcher", Gio.BusNameOwnerFlags.NONE,
                 on_bus, lambda c, n: print("watcher ready", flush=True), None)
loop = GLib.MainLoop()
signal.signal(signal.SIGTERM, lambda *_: loop.quit())
loop.run()
