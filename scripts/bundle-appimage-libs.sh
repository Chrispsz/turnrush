#!/usr/bin/env bash
# bundle-appimage-libs.sh — make the AppImage self-contained.
#
# Copies every shared library the game needs into <AppDir>/usr/lib so the
# binary runs on minimal / Wayland-only distros (e.g. Archcraft Elite, whose
# debloat leaves the X11 client stack incomplete: no libXcursor,
# libxkbcommon-x11, no fuse2 …).
#
# Bundled:
#   - full recursive closure of the binary's NEEDED libraries (ldd), and
#   - the libraries winit/x11-dl and xkbcommon load at RUNTIME via dlopen
#     (they never appear in ldd, so they must be listed explicitly).
#
# NOT bundled (must come from the host, by AppImage contract):
#   - glibc / core C runtime (libc, libm, libgcc_s …)
#   - graphics driver stack (libGL, libEGL, libvulkan, libdrm, libgbm,
#     nvidia…) — they must match the host GPU and kernel.
#
# Usage: bundle-appimage-libs.sh <AppDir> [path-to-binary]
set -euo pipefail

APPDIR="${1:?usage: bundle-appimage-libs.sh <AppDir> [binary]}"
BIN="${2:-$APPDIR/usr/bin/turnrush-game}"
LIBDIR="$APPDIR/usr/lib"
mkdir -p "$LIBDIR"
[ -x "$BIN" ] || { echo "binary not found: $BIN" >&2; exit 1; }

# dlopen-only dependencies (invisible to ldd):
#   - winit x11 backend via x11-dl: X11, X11-xcb, Xcursor, Xrandr, Xi,
#     Xinerama, xcb (+ xcb-util extensions used by x11rb), xkbcommon-x11
#   - xkbcommon via xkbcommon-dl: libxkbcommon
DLOPEN_LIST=(
  libX11.so.6 libX11-xcb.so.1 libxcb.so.1
  libXcursor.so.1 libXrandr.so.2 libXi.so.6 libXinerama.so.1
  libxkbcommon.so.0 libxkbcommon-x11.so.0
  libxcb-cursor.so.0 libxcb-render.so.0 libxcb-shape.so.0
  libxcb-xfixes.so.0 libxcb-xkb.so.1 libxcb-icccm.so.4
  libxcb-keysyms.so.1 libxcb-randr.so.0 libxcb-xinerama.so.0
)

# Returns 0 (skip) for libraries that must NOT be bundled.
exclude() {
  case "$1" in
    # core C runtime — always the host's
    ld-linux*|libc.so*|libm.so*|libdl*|libpthread*|librt.so*|libresolv*|\
    libutil*|libcrypt*|libgcc_s.so*|libstdc++*|libanl*|libBrokenLocale*) return 0 ;;
    # graphics/driver stack — must match host GPU and kernel
    libGL*|libEGL*|libGLES*|libvulkan*|libnvidia*|libdrm*|libgbm*|\
    libxatrack*|libvdpau*|libXvMC*|libOSMesa*|swrast*|libwayland-egl*) return 0 ;;
    *) return 1 ;;
  esac
}

find_lib() { # $1 = soname → prints host path of the real file
  local name="$1" hit d
  # optional extra dir (build sandboxes with partial dev trees)
  for d in ${TURNRUSH_LIB_EXTRA:-}; do
    [ -e "$d/$name" ] && { realpath "$d/$name"; return 0; }
  done
  hit="$(ldconfig -p 2>/dev/null | awk -v n="$name" '$1==n {print $NF; exit}')"
  if [ -n "$hit" ] && [ -f "$hit" ]; then realpath "$hit"; return 0; fi
  for d in /usr/lib/x86_64-linux-gnu /lib/x86_64-linux-gnu /usr/lib64 /usr/lib /lib; do
    [ -e "$d/$name" ] && { realpath "$d/$name"; return 0; }
  done
  return 1
}

SEEN=""
WORKLIST=("${DLOPEN_LIST[@]}")

# Seed the worklist with the binary's linked dependencies (ldd).
while IFS= read -r p; do
  [ -n "$p" ] && WORKLIST+=("$(basename "$p")")
done < <(ldd "$BIN" | awk '/=> \// {print $3} /^\// {print $1}')

copied=0
while [ "${#WORKLIST[@]}" -gt 0 ]; do
  name="${WORKLIST[0]}"; WORKLIST=("${WORKLIST[@]:1}")
  case " $SEEN " in *" $name "*) continue ;; esac
  SEEN="$SEEN $name"
  exclude "$name" && continue
  [ -e "$LIBDIR/$name" ] && continue
  src="$(find_lib "$name")" || { echo "ERROR: host library not found: $name" >&2; exit 1; }
  cp -L "$src" "$LIBDIR/$name"
  copied=$((copied + 1))
  # transitive dependencies of the just-copied library
  while IFS= read -r p; do
    [ -n "$p" ] && WORKLIST+=("$(basename "$p")")
  done < <(ldd "$LIBDIR/$name" | awk '/=> \// {print $3} /^\// {print $1}')
done

echo "bundled $copied libraries into $LIBDIR:"
(ls "$LIBDIR" | sort | tr '\n' ' '; echo) | sed 's/^/  /'

# Verification: with the bundled dir first, nothing may be unresolved.
missing="$(LD_LIBRARY_PATH="$LIBDIR" ldd "$BIN" | grep 'not found' || true)"
if [ -n "$missing" ]; then
  echo "ERROR: unresolved libraries after bundling:" >&2
  echo "$missing" >&2
  exit 1
fi
echo "verification OK: all dependencies resolve (bundled or host core)."
