#!/usr/bin/env python3
"""Exercise a built Phase 1 app through its real WebDriver and Tauri IPC.

Use an isolated, empty XDG_DATA_HOME on Linux, or an isolated Windows user.
Start tauri-driver separately; it starts the application for each session.
This uses only Python's standard library and adds no application test hooks.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import socket
import sqlite3
import subprocess
import sys
import time
from urllib.error import HTTPError
from urllib.request import Request, urlopen
import uuid


class Driver:
    def __init__(self, url: str, application: Path):
        self.url = url.rstrip("/")
        self.application = application
        self.session: str | None = None

    def request(self, method: str, path: str, payload=None):
        data = None if payload is None else json.dumps(payload).encode()
        req = Request(self.url + path, data, {"Content-Type": "application/json"}, method=method)
        try:
            with urlopen(req, timeout=180 if method == "POST" and path == "/session" else 45) as response:
                result = json.load(response)
        except HTTPError as error:
            raise RuntimeError(f"WebDriver {method} {path}: {error.read().decode()}") from error
        value = result.get("value", result)
        if isinstance(value, dict) and "error" in value and "ok" not in value:
            raise RuntimeError(f"WebDriver {method} {path}: {value}")
        return value

    def start(self):
        result = self.request("POST", "/session", {"capabilities": {"alwaysMatch": {
            "tauri:options": {"application": str(self.application)}
        }}})
        self.session = result["sessionId"]
        self.request("POST", self.path("/timeouts"), {"script": 15000})

    def path(self, suffix: str):
        if self.session is None:
            raise RuntimeError("No active WebDriver session")
        return f"/session/{self.session}{suffix}"

    def script(self, script: str, *args):
        return self.request("POST", self.path("/execute/sync"), {"script": script, "args": list(args)})

    def invoke(self, command: str, args=None):
        return self.request("POST", self.path("/execute/async"), {
            "script": """
                const done = arguments[arguments.length - 1];
                window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1])
                    .then(value => done({ok: true, value}))
                    .catch(error => done({ok: false, error: String(error)}));
            """, "args": [command, args or {}]
        })

    def select(self, label: str):
        handles = self.request("GET", self.path("/window/handles"))
        labels = []
        for handle in handles:
            self.request("POST", self.path("/window"), {"handle": handle})
            observed = self.script("return document.documentElement.dataset.window || null")
            labels.append(observed)
            if observed == label:
                return
        raise RuntimeError(f"Cannot select window {label!r}; WebDriver exposes {labels!r}")

    def screenshot(self, path: Path):
        self.request("POST", self.path("/execute/async"), {
            "script": "const done = arguments[0]; requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(done, 150)));",
            "args": []
        })
        value = self.request("GET", self.path("/screenshot"))
        path.write_bytes(base64.b64decode(value))

    def dispose(self):
        if self.session is not None:
            try:
                self.request("DELETE", self.path(""))
            except (RuntimeError, OSError):
                pass
            self.session = None


def wait_for(callback, message: str, seconds: float = 15):
    deadline = time.monotonic() + seconds
    last_error = None
    while time.monotonic() < deadline:
        try:
            result = callback()
            if result:
                return result
        except (RuntimeError, OSError) as error:
            last_error = error
        time.sleep(0.15)
    raise RuntimeError(f"{message}; last error: {last_error}")


def native_pids(application: Path):
    """Linux-only evidence; Windows process checks remain manual."""
    if sys.platform != "linux":
        return None
    result = []
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                if (entry / "exe").resolve(strict=True) == application:
                    result.append(int(entry.name))
            except (OSError, RuntimeError):
                pass
    return sorted(result)


def require(condition, message: str):
    if not condition:
        raise RuntimeError(message)


def loopback_port_open(port: int):
    with socket.socket() as connection:
        connection.settimeout(0.5)
        return connection.connect_ex(("127.0.0.1", port)) == 0


def native_window(label: str):
    title = {"pet": "Character", "panel": "Panel", "settings": "Settings"}[label]
    return subprocess.check_output(["xdotool", "search", "--name", f"^MARKETING MATE.*{title}$"],
                                   text=True).splitlines()[0]


def window_visible(label: str):
    info = subprocess.check_output(["xwininfo", "-id", native_window(label)], text=True)
    return "Map State: IsViewable" in info


def dbus_call(destination: str, object_path: str, method: str, *args: str):
    return subprocess.check_output(["gdbus", "call", "--session", "--dest", destination,
                                    "--object-path", object_path, "--method", method,
                                    "--", *args], text=True, stderr=subprocess.STDOUT)


def tray_menu():
    names = dbus_call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus.ListNames")
    destination = next(iter(re.findall(r"'(org\.kde\.StatusNotifierItem[^']+)'", names)), None)
    status_path = "/StatusNotifierItem"
    if destination is None:
        registered = dbus_call("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher",
                               "org.freedesktop.DBus.Properties.Get", "org.kde.StatusNotifierWatcher",
                               "RegisteredStatusNotifierItems")
        item = re.search(r"'(:\d+\.\d+)(/[^']+)'", registered)
        if item is None:
            raise RuntimeError(f"No StatusNotifierItem service was registered: {registered}")
        destination, status_path = item[1], item[2]
    prop = dbus_call(destination, status_path, "org.freedesktop.DBus.Properties.Get",
                     "org.kde.StatusNotifierItem", "Menu")
    object_path = re.search(r"objectpath '([^']+)'", prop)
    if object_path is None:
        raise RuntimeError(f"Cannot find native tray menu object: {prop}")
    layout = dbus_call(destination, object_path[1], "com.canonical.dbusmenu.GetLayout", "0", "-1", "[]")
    item_ids = {}
    for label in ["패널 열기", "설정", "캐릭터 표시", "캐릭터 숨기기", "종료"]:
        item = re.search(r"\((\d+), \{[^}]*'label': <'" + label + r"'>", layout)
        if item is None:
            raise RuntimeError(f"Missing tray menu item {label}: {layout}")
        item_ids[label] = item[1]
    return destination, object_path[1], item_ids, layout


def tray_click(menu, label: str):
    return dbus_call(menu[0], menu[1], "com.canonical.dbusmenu.Event", menu[2][label],
                     "clicked", "<0>", "0")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://127.0.0.1:4444")
    parser.add_argument("--application", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path(".verification/desktop"))
    parser.add_argument("--database", type=Path, help="Isolated smoke profile's SQLite path, for readonly checks")
    parser.add_argument("--corrupt-fixture", action="store_true",
                        help="Check corrupt-DB diagnostics; requires an isolated database inside OUTPUT/profile")
    parser.add_argument("--require-tray", action="store_true", help="Fail if native Linux tray checks cannot run")
    parser.add_argument("--development-only", action="store_true",
                        help="Launch npm run desktop:dev and verify its Vite-backed native UI only")
    options = parser.parse_args()
    application = options.application.resolve(strict=True)
    output = options.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    driver = Driver(options.url, application)
    report = {"platform": sys.platform, "application": str(application), "checks": [],
              "limits": ["Windows behavior requires a Windows 11 x64 smoke run.",
                         "Phase 2 character interactions and final asset acceptance are excluded."]}

    def record(name: str, detail=None):
        report["checks"].append({"name": name, "status": "passed", "detail": detail})
        print(f"PASS {name}", flush=True)

    def select_loaded(label: str):
        def loaded():
            driver.select(label)
            return driver.script("return !!document.querySelector('#root')?.children.length")
        wait_for(loaded, f"React UI did not load in {label}")

    try:
        require(not native_pids(application), "An existing app is running; use an isolated session")
        if options.development_only:
            require(not loopback_port_open(1420), "Vite port 1420 is already occupied")
            launcher = output / "desktop-dev-launcher.sh"
            exit_file = output / "desktop-dev-exit-code"
            repo = Path(__file__).resolve().parent.parent
            launcher.write_text("#!/usr/bin/env bash\nset -euo pipefail\nexport UV_USE_IO_URING=0\ncd " + shlex.quote(str(repo)) +
                                "\nset +e\nnpm run desktop:dev\ncli_status=$?\nprintf '%s\\n' \"$cli_status\" > " +
                                shlex.quote(str(exit_file)) + "\nexit \"$cli_status\"\n")
            launcher.chmod(0o700)
            driver.application = launcher
            driver.start()
            select_loaded("pet")
            location = driver.script("return location.href")
            require(location.startswith("http://127.0.0.1:1420/"), f"Development UI did not use Vite: {location}")
            with urlopen("http://127.0.0.1:1420/", timeout=5) as response:
                require(response.status == 200, "Vite did not serve the frontend")
            record("npm run desktop:dev starts Vite and loads native React pet", location)
            require(driver.invoke("open_settings")["ok"], "Development pet could not open settings")
            select_loaded("settings")
            status = driver.invoke("get_app_status")
            profile = driver.invoke("get_profile")
            require(status["ok"] and status["value"]["database_ready"] and profile["ok"],
                    f"Development real IPC failed: {status}, {profile}")
            wait_for(lambda: driver.script("return document.body.innerText.includes('로컬 DB 연결됨')"),
                     "Development Settings React did not display DB status")
            driver.screenshot(output / "settings-development.png")
            record("Vite-backed native Settings reaches real Rust IPC", status["value"])
            try:
                driver.invoke("quit_app")
            except (RuntimeError, OSError):
                pass
            wait_for(lambda: native_pids(application) == [], "Development app did not exit after explicit Quit")
            record("development native app exits cleanly")
            wait_for(exit_file.exists, "Development CLI did not finish after app Quit", seconds=15)
            cli_status = int(exit_file.read_text().strip())
            report["development_cli_exit_code"] = cli_status
            require(cli_status == 0, f"npm run desktop:dev exited {cli_status} after app Quit; see driver.log")
            wait_for(lambda: not loopback_port_open(1420), "Vite remained running after development CLI exit")
            record("development CLI exits zero and stops Vite", {"exit_code": cli_status, "port_1420_open": False})
            report["status"] = "passed"
            return 0
        driver.start()
        select_loaded("pet")
        pet = driver.script("""return {
            label: document.documentElement.dataset.window,
            background: getComputedStyle(document.documentElement).backgroundColor,
            bodyBackground: getComputedStyle(document.body).backgroundColor,
            text: document.body.innerText,
            innerWidth, innerHeight,
            devicePixelRatio,
            bodyMinWidth: getComputedStyle(document.body).minWidth,
            canvasWidth: getComputedStyle(document.querySelector('#root')).width,
            canvasHeight: getComputedStyle(document.querySelector('#root')).height,
            surfaceWidth: getComputedStyle(document.querySelector('.pet-surface')).width,
            surfaceHeight: getComputedStyle(document.querySelector('.pet-surface')).height,
            placeholder: document.querySelector('[role="img"]')?.getAttribute('aria-label')
        }""")
        report["pet_observation"] = pet
        require(pet["background"] == "rgba(0, 0, 0, 0)" and
                pet["bodyBackground"] == "rgba(0, 0, 0, 0)", "Pet document is not transparent")
        require("DEVELOPMENT ASSET" in pet["text"], "Missing development asset disclosure")
        require(pet["canvasWidth"] == "192px" and pet["canvasHeight"] == "192px", "Unexpected pet canvas size")
        if pet["innerWidth"] != 192 or pet["innerHeight"] != 192:
            report["limits"].append(f"Native {sys.platform} viewport is {pet['innerWidth']}x{pet['innerHeight']} "
                                     "although the configured and CSS canvas size is 192x192.")
        record("loaded React pet, transparent root, development placeholder", pet)
        driver.screenshot(output / "pet.png")
        if sys.platform == "linux":
            hints = subprocess.check_output(["xprop", "-id", native_window("pet"),
                                             "_MOTIF_WM_HINTS", "_NET_WM_STATE"], text=True)
            require("0x2, 0x0, 0x0" in hints, f"Pet native decorations are not disabled: {hints}")
            require("_NET_WM_STATE_ABOVE" in hints and "_NET_WM_STATE_SKIP_TASKBAR" in hints,
                    f"Missing pet desktop state: {hints}")
            record("native pet frameless, above, skip taskbar", hints)
        denied = driver.invoke("get_profile")
        require(not denied["ok"], "Pet obtained the personal profile despite ACL")
        record("pet profile access denied", denied)
        require(driver.invoke("open_settings")["ok"], "Pet could not open settings")
        select_loaded("settings")
        status = driver.invoke("get_app_status")
        require(status["ok"] and status["value"]["database_ready"], f"Database not ready: {status}")
        profile = driver.invoke("get_profile")
        require(profile["ok"], f"Profile unavailable: {profile}")
        first_profile_id = profile["value"]["profile_id"]
        require(uuid.UUID(first_profile_id).version == 4, "Profile identifier is not UUID v4")
        wait_for(lambda: driver.script("return document.body.innerText.includes('로컬 DB 연결됨')"),
                 "Settings does not show connected database")
        ui = driver.script("""return {
            text: document.body.innerText,
            readonly: document.querySelector('.readonly-tag')?.textContent,
            editable: document.querySelectorAll('input,textarea,[contenteditable="true"]').length,
            disabledFutureSections: document.querySelectorAll('.nav-item--disabled:disabled').length
        }""")
        require(ui["readonly"] == "읽기 전용" and ui["editable"] == 0, "Settings profile is unexpectedly editable")
        require(ui["disabledFutureSections"] == 8, "Future sections are unexpectedly active")
        require(profile["value"]["user_name"] in ui["text"], "Profile not rendered by React")
        record("settings real Rust IPC and readonly profile", {"status": status["value"], "profile_id": first_profile_id})
        driver.screenshot(output / "settings.png")
        for command, args in [
            ("execute_sql", {"sql": "DROP TABLE personal_profile"}),
            ("plugin:sql|execute", {"db": "sqlite:marketing-mate.sqlite3", "query": "DROP TABLE personal_profile"}),
            ("read_file", {"path": "/etc/passwd"}),
            ("plugin:fs|read_text_file", {"path": "/etc/passwd"}),
        ]:
            result = driver.invoke(command, args)
            require(not result["ok"], f"Unexpected arbitrary operation exposed: {command}")
            record(f"arbitrary operation rejected: {command}", result)
        require(driver.invoke("open_panel")["ok"], "Could not open panel")
        select_loaded("panel")
        denied = driver.invoke("get_profile")
        require(not denied["ok"], "Panel obtained profile despite ACL")
        record("panel profile access denied", denied)
        driver.screenshot(output / "panel.png")
        require(driver.invoke("hide_pet")["ok"], "Panel could not hide pet")
        if sys.platform == "linux":
            wait_for(lambda: not window_visible("pet"), "Pet remains mapped after hide")
        require(driver.invoke("show_pet")["ok"], "Panel could not recover pet")
        if sys.platform == "linux":
            wait_for(lambda: window_visible("pet"), "Pet remains hidden after show")
        record("typed hide/show pet commands succeed")
        menu = None
        if sys.platform == "linux":
            try:
                menu = tray_menu()
                (output / "tray-layout.txt").write_text(menu[3])
                tray_click(menu, "캐릭터 숨기기")
                wait_for(lambda: not window_visible("pet"), "Tray callback did not hide pet")
                tray_click(menu, "캐릭터 표시")
                wait_for(lambda: window_visible("pet"), "Tray callback did not recover pet")
                tray_click(menu, "설정")
                wait_for(lambda: window_visible("settings"), "Tray callback did not open settings")
                tray_click(menu, "패널 열기")
                wait_for(lambda: window_visible("panel"), "Tray callback did not open panel")
                record("registered Linux tray menu and native callbacks", menu[2])
                report["limits"].append("Linux tray callbacks were invoked through their native DBus menu; "
                                         "Windows rendered tray mouse interaction remains pending.")
            except (RuntimeError, OSError, subprocess.SubprocessError) as error:
                report["checks"].append({"name": "registered Linux tray menu and native callbacks",
                                         "status": "failed" if options.require_tray else "skipped",
                                         "detail": str(error)})
                if options.require_tray:
                    raise RuntimeError(f"Required native Linux tray checks failed: {error}") from error
                report["limits"].append(f"Linux tray menu validation incomplete: {error}")
                menu = None
            before = native_pids(application)
            require(len(before) == 1, f"Expected one app process: {before}")
            subprocess.run(["xdotool", "windowactivate", "--sync", native_window("panel"),
                            "key", "--clearmodifiers", "alt+F4"], check=True)
            wait_for(lambda: not window_visible("panel"), "Panel did not hide after native close")
            require(native_pids(application) == before, "Closing the panel exited the app")
            record("native panel close hides window and keeps app running")
            environment = os.environ.copy()
            environment["TAURI_WEBVIEW_AUTOMATION"] = "false"
            with (output / "second-instance.log").open("wb") as log:
                second = subprocess.run([str(application)], env=environment, stdout=log, stderr=log, timeout=15)
            require(second.returncode == 0, f"Second instance exited {second.returncode}")
            require(native_pids(application) == before, "Second launch created a duplicate app process")
            wait_for(lambda: window_visible("panel"), "Second launch did not recover the existing panel")
            record("second instance exits and preserves original process", {"pids": before, "exit_code": second.returncode})
        else:
            report["limits"].append("Second-instance process count was not automated on this platform.")
        select_loaded("settings")
        try:
            if menu:
                tray_click(menu, "종료")
            else:
                driver.invoke("quit_app")
        except (RuntimeError, OSError, subprocess.SubprocessError):
            # Explicit Quit may close IPC before its reply is returned.
            pass
        if sys.platform == "linux":
            wait_for(lambda: native_pids(application) == [], "App did not exit after explicit Quit")
        record("explicit native tray Quit exits the application" if menu else "explicit Quit exits the application")
        driver.dispose()
        driver.start()
        select_loaded("pet")
        require(driver.invoke("open_settings")["ok"], "Could not open settings after relaunch")
        select_loaded("settings")
        second_profile = driver.invoke("get_profile")
        require(second_profile["ok"] and second_profile["value"]["profile_id"] == first_profile_id,
                "Relaunch changed or lost the profile")
        record("relaunch preserves profile UUID", first_profile_id)
        if options.database:
            database = options.database.resolve(strict=True)
            connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
            try:
                require(connection.execute("PRAGMA integrity_check").fetchone()[0] == "ok", "SQLite integrity failure")
                require(connection.execute("SELECT count(*) FROM personal_profile").fetchone()[0] == 1,
                        "Profile is not a singleton")
                migrations = connection.execute("SELECT version, checksum FROM schema_migrations ORDER BY version").fetchall()
                require(len(migrations) == 1 and migrations[0][0] == 1, "Unexpected Phase 1 migration history")
                require(len(migrations[0][1]) == 64, "Missing SHA-256 migration checksum")
                record("readonly persisted SQLite integrity, singleton, migration", {"migrations": migrations})
            finally:
                connection.close()
        try:
            driver.invoke("quit_app")
        except (RuntimeError, OSError):
            pass
        if sys.platform == "linux":
            wait_for(lambda: native_pids(application) == [], "Relaunched app did not exit")
        record("relaunch exits cleanly")
        if options.corrupt_fixture:
            require(options.database is not None, "Corrupt fixture check requires --database")
            database = options.database.resolve(strict=True)
            require(database.is_relative_to(output / "profile"),
                    "Refusing to write a DB outside the isolated OUTPUT/profile smoke fixture")
            driver.dispose()
            original = database.read_bytes()
            damaged = b"MARKETING MATE smoke-only invalid SQLite fixture\n"
            database.write_bytes(damaged)
            try:
                driver.start()
                select_loaded("settings")
                diagnostic = driver.invoke("get_app_status")
                require(diagnostic["ok"] and diagnostic["value"]["database_ready"] is False,
                        "Corrupt database was reported ready")
                require(diagnostic["value"]["error_code"] == "DB_CORRUPT",
                        f"Unexpected corruption diagnostic: {diagnostic}")
                unavailable = driver.invoke("get_profile")
                require(not unavailable["ok"] and "DB_CORRUPT" in unavailable["error"],
                        "Corrupt database exposed or recreated a profile")
                wait_for(lambda: driver.script("return document.body.innerText.includes('DB_CORRUPT')"),
                         "Corrupt database diagnostic is absent from Settings UI")
                driver.screenshot(output / "settings-corrupt-database.png")
                require(database.read_bytes() == damaged, "Startup changed the original corrupt fixture")
                record("corrupt DB preserves original bytes and opens Settings diagnosis", diagnostic["value"])
                try:
                    driver.invoke("quit_app")
                except (RuntimeError, OSError):
                    pass
                if sys.platform == "linux":
                    wait_for(lambda: native_pids(application) == [], "Diagnostic app did not exit")
                driver.dispose()
            finally:
                driver.dispose()
                require(not native_pids(application), "Refusing fixture restoration while app remains running")
                database.write_bytes(original)
                report["fixture_restored_sha256"] = hashlib.sha256(original).hexdigest()
            record("isolated corrupt fixture restored after clean exit")
        report["status"] = "passed"
    except Exception as error:
        report["status"] = "failed"
        report["failure"] = str(error)
        print(f"FAIL {error}", file=sys.stderr, flush=True)
        return 1
    finally:
        driver.dispose()
        (output / "results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
