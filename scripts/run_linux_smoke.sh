#!/usr/bin/env bash
# Local Linux cloud verification only. Windows acceptance must run on Windows.
set -euo pipefail
repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_dir"
. "${MARKETING_MATE_LINUX_ENV:-/workspace/.tools/linux-env.sh}"
export PATH="/workspace/.tools/rust/bin:/workspace/.tools/tauri-driver/bin:$PATH"
export CARGO_HOME="${CARGO_HOME:-/workspace/.cache/cargo}"
export npm_config_cache="${npm_config_cache:-/workspace/.cache/npm}"
export PYTHONDONTWRITEBYTECODE=1
# libuv's supported filesystem fallback avoids io_uring/proot address translation.
export UV_USE_IO_URING=0

if [[ ${MARKETING_MATE_SMOKE_SESSION:-0} != 1 ]]; then
  mkdir -p .verification
  run_dir="$(mktemp -d "$repo_dir/.verification/linux-smoke-XXXXXX")"
  export MARKETING_MATE_SMOKE_OUTPUT="$run_dir"
  export XDG_DATA_HOME="$run_dir/profile/data"
  export XDG_CONFIG_HOME="$run_dir/profile/config"
  export XDG_CACHE_HOME="$run_dir/profile/cache"
  export XDG_RUNTIME_DIR="$run_dir/profile/runtime"
  mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
  chmod 700 "$XDG_RUNTIME_DIR"
  export MARKETING_MATE_SMOKE_SESSION=1
  printf 'Linux smoke artifacts: %s\n' "$run_dir"
  if [[ " $* " == *" --development-only "* ]]; then
    # Keep Node/Vite/Cargo outside proot; bind WebKit helpers only for the app.
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="$(command -v with-local-webkit)"
    exec dbus-run-session -- bash "$0" "$@"
  fi
  exec dbus-run-session -- with-local-webkit bash "$0" "$@"
fi

run_dir="$MARKETING_MATE_SMOKE_OUTPUT"
processes=()
cleanup() {
  trap - EXIT INT TERM
  for ((i=${#processes[@]}-1; i>=0; i--)); do
    kill -TERM -- "-${processes[i]}" 2>/dev/null || true
    wait "${processes[i]}" 2>/dev/null || true
  done
}
trap cleanup EXIT INT TERM
for command in Xvfb openbox picom cc pkg-config xdotool xwininfo xprop gdbus tauri-driver WebKitWebDriver setsid; do
  command -v "$command" >/dev/null || { printf 'Missing prerequisite: %s\n' "$command" >&2; exit 1; }
done

setsid Xvfb -displayfd 3 -screen 0 1280x900x24 +extension COMPOSITE 3>"$run_dir/display" >"$run_dir/xvfb.log" 2>&1 &
processes+=("$!")
for attempt in {1..50}; do
  [[ -s "$run_dir/display" ]] && break
  sleep .1
done
[[ -s "$run_dir/display" ]] || { printf 'Xvfb did not allocate a display\n' >&2; exit 1; }
export DISPLAY=":$(cat "$run_dir/display")"
setsid openbox >"$run_dir/openbox.log" 2>&1 &
processes+=("$!")
setsid picom --backend xrender >"$run_dir/picom.log" 2>&1 &
processes+=("$!")
# Provide a minimal StatusNotifier desktop host for native tray-menu verification.
# The fixture is outside the application; no application commands or ACLs change.
cat >"$run_dir/status-watcher.c" <<'WATCHER'
#include <gio/gio.h>
static gchar *item = NULL;
static const gchar xml[] =
"<node><interface name='org.kde.StatusNotifierWatcher'>"
"<method name='RegisterStatusNotifierItem'><arg type='s' direction='in'/></method>"
"<method name='RegisterStatusNotifierHost'><arg type='s' direction='in'/></method>"
"<property name='RegisteredStatusNotifierItems' type='as' access='read'/>"
"<property name='IsStatusNotifierHostRegistered' type='b' access='read'/>"
"<property name='ProtocolVersion' type='i' access='read'/>"
"<signal name='StatusNotifierItemRegistered'><arg type='s'/></signal>"
"<signal name='StatusNotifierHostRegistered'/>"
"</interface></node>";
static void method(GDBusConnection *c,const gchar *sender,const gchar *path,const gchar *iface,const gchar *name,GVariant *args,GDBusMethodInvocation *inv,gpointer data) {
 if(g_str_equal(name,"RegisterStatusNotifierItem")) {
  const gchar *service; g_variant_get(args,"(&s)",&service);
  g_free(item); item=service[0]=='/' ? g_strdup_printf("%s%s",sender,service) : g_strdup(service);
  g_dbus_connection_emit_signal(c,NULL,"/StatusNotifierWatcher","org.kde.StatusNotifierWatcher","StatusNotifierItemRegistered",g_variant_new("(s)",item),NULL);
  g_print("REGISTERED %s\n",item);
 }
 g_dbus_method_invocation_return_value(inv,NULL);
}
static GVariant *property(GDBusConnection *c,const gchar *sender,const gchar *path,const gchar *iface,const gchar *name,GError **err,gpointer data) {
 if(g_str_equal(name,"IsStatusNotifierHostRegistered")) return g_variant_new_boolean(TRUE);
 if(g_str_equal(name,"ProtocolVersion")) return g_variant_new_int32(0);
 const gchar *items[]={item,NULL}; return g_variant_new_strv(items,item ? 1 : 0);
}
static const GDBusInterfaceVTable vtable={method,property,NULL};
int main(void) {
 GError *error=NULL;
 GDBusConnection *c=g_bus_get_sync(G_BUS_TYPE_SESSION,NULL,&error);
 if(!c) { g_printerr("%s\n",error->message); return 1; }
 GDBusNodeInfo *info=g_dbus_node_info_new_for_xml(xml,&error);
 if(!g_dbus_connection_register_object(c,"/StatusNotifierWatcher",info->interfaces[0],&vtable,NULL,NULL,&error)) { g_printerr("%s\n",error->message); return 1; }
 g_bus_own_name_on_connection(c,"org.kde.StatusNotifierWatcher",G_BUS_NAME_OWNER_FLAGS_NONE,NULL,NULL,NULL,NULL);
 g_main_loop_run(g_main_loop_new(NULL,FALSE));
 return 0;
}
WATCHER
cc "$run_dir/status-watcher.c" -o "$run_dir/status-watcher" $(pkg-config --cflags --libs gio-2.0)
setsid "$run_dir/status-watcher" >"$run_dir/status-watcher.log" 2>&1 &
processes+=("$!")
python3 - <<'WAIT_WATCHER'
import subprocess,time
for _ in range(50):
 result=subprocess.run(['gdbus','call','--session','--dest','org.freedesktop.DBus','--object-path','/org/freedesktop/DBus','--method','org.freedesktop.DBus.NameHasOwner','org.kde.StatusNotifierWatcher'],capture_output=True,text=True)
 if '(true,)' in result.stdout: break
 time.sleep(.1)
else: raise RuntimeError('StatusNotifier fixture did not start')
WAIT_WATCHER
# Select distinct ephemeral ports; only loopback binds are used.
read -r driver_port native_port < <(python3 - <<'PORTS'
import socket
with socket.socket() as first, socket.socket() as second:
 first.bind(('127.0.0.1',0));second.bind(('127.0.0.1',0))
 print(first.getsockname()[1],second.getsockname()[1])
PORTS
)
setsid tauri-driver --port "$driver_port" --native-port "$native_port" --native-driver "$(command -v WebKitWebDriver)" >"$run_dir/driver.log" 2>&1 &
processes+=("$!")
python3 - "$driver_port" <<'WAIT_DRIVER'
import sys,time,urllib.request
for _ in range(50):
 try:
  urllib.request.urlopen('http://127.0.0.1:'+sys.argv[1]+'/status',timeout=1)
  break
 except OSError: time.sleep(.1)
else: raise RuntimeError('tauri-driver did not start')
WAIT_DRIVER
python3 scripts/smoke_desktop.py --url "http://127.0.0.1:$driver_port" --application src-tauri/target/debug/marketing-mate --output "$run_dir" --database "$XDG_DATA_HOME/com.marketingmate.desktop/marketing-mate.sqlite3" --corrupt-fixture --require-tray "$@"
printf 'Linux smoke results: %s/results.json\n' "$run_dir"
