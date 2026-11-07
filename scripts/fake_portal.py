#!/usr/bin/env python3
"""A stand-in for the XDG GlobalShortcuts portal, for uttr's Wayland test in CI.

It owns org.freedesktop.portal.Desktop on the session bus and answers CreateSession and
BindShortcuts like xdg-desktop-portal does (a Request object path plus a Response signal).
After binding it plays a fixed script of Activated/Deactivated signals: dictate held for
600 ms, then command held for 600 ms. The preferred triggers it was given are written to
$FAKE_PORTAL_LOG.
"""
import json
import os

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

IFACE = "org.freedesktop.portal.GlobalShortcuts"
ROOT = "/org/freedesktop/portal/desktop"

DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()


def sender_id(sender):
    return sender.lstrip(":").replace(".", "_")


class Request(dbus.service.Object):
    @dbus.service.signal("org.freedesktop.portal.Request", signature="ua{sv}")
    def Response(self, code, results):
        pass


class Portal(dbus.service.Object):
    def __init__(self):
        super().__init__(bus, ROOT)
        self.keep = []

    def reply_later(self, sender, token, results, then=None):
        req = Request(bus, f"{ROOT}/request/{sender_id(sender)}/{token}")
        self.keep.append(req)

        def fire():
            req.Response(dbus.UInt32(0), dbus.Dictionary(results, signature="sv"))
            if then:
                then()
            return False

        GLib.timeout_add(50, fire)
        return dbus.ObjectPath(req._object_path)

    @dbus.service.method(IFACE, in_signature="a{sv}", out_signature="o", sender_keyword="sender")
    def CreateSession(self, options, sender=None):
        session = f"{ROOT}/session/{sender_id(sender)}/{options['session_handle_token']}"
        return self.reply_later(sender, options["handle_token"], {"session_handle": dbus.ObjectPath(session)})

    @dbus.service.method(IFACE, in_signature="oa(sa{sv})sa{sv}", out_signature="o", sender_keyword="sender")
    def BindShortcuts(self, session, shortcuts, parent, options, sender=None):
        triggers = {str(sid): str(info.get("preferred_trigger", "")) for sid, info in shortcuts}
        log = os.environ.get("FAKE_PORTAL_LOG")
        if log:
            with open(log, "w") as f:
                json.dump(triggers, f)
        bound = dbus.Array(
            [dbus.Struct((sid, dbus.Dictionary({"description": "test", "trigger_description": t}, signature="sv")))
             for sid, t in triggers.items()],
            signature="(sa{sv})",
        )
        return self.reply_later(sender, options["handle_token"], {"shortcuts": bound}, then=lambda: self.play(session))

    def play(self, session):
        script = [(300, "dictate", True), (900, "dictate", False), (1800, "command", True), (2400, "command", False)]
        for ms, sid, on in script:
            def emit(sid=sid, on=on):
                args = (dbus.ObjectPath(session), sid, dbus.UInt64(0), dbus.Dictionary({}, signature="sv"))
                (self.Activated if on else self.Deactivated)(*args)
                return False
            GLib.timeout_add(ms, emit)

    @dbus.service.signal(IFACE, signature="osta{sv}")
    def Activated(self, session, shortcut_id, timestamp, options):
        pass

    @dbus.service.signal(IFACE, signature="osta{sv}")
    def Deactivated(self, session, shortcut_id, timestamp, options):
        pass

    @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="ss", out_signature="v")
    def Get(self, interface, prop):
        if interface == IFACE and prop == "version":
            return dbus.UInt32(1)
        raise dbus.exceptions.DBusException("org.freedesktop.DBus.Error.InvalidArgs", f"no {interface}.{prop}")

    @dbus.service.method(dbus.PROPERTIES_IFACE, in_signature="s", out_signature="a{sv}")
    def GetAll(self, interface):
        return dbus.Dictionary({"version": dbus.UInt32(1)} if interface == IFACE else {}, signature="sv")


name = dbus.service.BusName("org.freedesktop.portal.Desktop", bus)
portal = Portal()
print("fake GlobalShortcuts portal ready", flush=True)
GLib.MainLoop().run()
