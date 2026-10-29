#!/bin/sh
# turnrush-doctor — diagnóstico de compatibilidade do TurnRush (pt-BR).
#
# Verifica NO HOST tudo o que o AppImage/portável não podem embutir:
#   1. FUSE2        — sem ele o AppImage não abre com duplo-clique;
#   2. sessão gráfica — Wayland ou X11 acessível;
#   3. GPU/Vulkan   — drivers e ICDs que o wgpu precisa (não embutíveis);
#   4. bibliotecas  — varre o binário real (ldd) e mapeia faltantes p/ pacotes.
#
# Uso:
#   ./turnrush-doctor.sh                       # dentro de um dir com TurnRush*.AppImage
#   ./turnrush-doctor.sh TurnRush-0.10.0-x86_64.AppImage
#   ./turnrush-doctor.sh ./turnrush-game       # binário solto (tar.gz portável/build)
#
# Saída: resumo ✅/⚠️/❌ + comandos prontos para pacman/apt/dnf. Exit 0 = OK.
set -u

APPIMAGE_HINT=""
TARGET="${1:-}"
if [ -z "$TARGET" ]; then
  for f in TurnRush-*.AppImage ./TurnRush-*.AppImage; do
    [ -f "$f" ] && TARGET="$f" && break
  done
fi

OK="✅"; WARN="⚠️"; FAIL="❌"
problems=0
summary=""

add() { # $1 level, $2 text
  summary="$summary
$1 $2"
}

pkg_cmd() { # $* package names → commando de instalação da distro detectada
  local pkgs="$*" id=""
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    id="$(. /etc/os-release && printf '%s' "${ID:-}")"
  fi
  case "$id" in
    arch|cachyos|archcraft|artix|endeavouros|garuda|manjaro)
      echo "sudo pacman -S --needed $pkgs" ;;
    ubuntu|debian|linuxmint|pop|neon)
      echo "sudo apt-get install -y $pkgs" ;;
    fedora)
      echo "sudo dnf install -y $pkgs" ;;
    opensuse*|suse)
      echo "sudo zypper install $pkgs" ;;
    *)
      echo "instale os pacotes: $pkgs" ;;
  esac
}

# package name equivalents: arch | debian | fedora
pkg_for() {
  case "$1" in
    libX11.so.6)        echo "libx11 libx11-6 libX11" ;;
    libX11-xcb.so.1)    echo "libx11 libx11-6 libX11" ;;
    libxcb.so.1)        echo "libxcb libxcb1 libxcb" ;;
    libxcb-*.so*)       echo "libxcb libxcb1 libxcb" ;;
    libXcursor.so.1)    echo "libxcursor libxcursor1 libXcursor" ;;
    libXrandr.so.2)     echo "libxrandr libxrandr2 libXrandr" ;;
    libXi.so.6)         echo "libxi libxi6 libXi" ;;
    libXinerama.so.1)   echo "libxinerama libxinerama1 libXinerama" ;;
    libxkbcommon.so.0)  echo "libxkbcommon libxkbcommon0 libxkbcommon" ;;
    libxkbcommon-x11.so.0) echo "libxkbcommon-x11 libxkbcommon-x11-0 libxkbcommon-x11" ;;
    libwayland-*.so*)   echo "wayland libwayland-client0 libwayland" ;;
    libvulkan.so.1)     echo "vulkan-icd-loader libvulkan1 vulkan-loader" ;;
    libGL.so.1)         echo "mesa libgl1 mesa-libGL" ;;
    libEGL.so.1)        echo "mesa libegl1 mesa-libEGL" ;;
    libasound.so.2)     echo "alsa-lib libasound2 alsa-lib" ;;
    libudev.so.1)       echo "systemd-libs libudev1 systemd-libs" ;;
    libexpat.so.1)      echo "expat libexpat1 expat" ;;
    libglib-2.0.so.0)   echo "glib2 libglib2.0-0 glib2" ;;
    *)                  echo "$1 (procure no gerenciador de pacotes)" ;;
  esac
}

echo "╭─────────────────────────────────────────────╮"
echo "│ TurnRush Doctor — diagnóstico de sistema    │"
echo "╰─────────────────────────────────────────────╯"
echo "Alvo: ${TARGET:-<nenhum — checando só o sistema>}"
echo ""

# ── 1. FUSE2 (AppImage precisa para abrir direto) ──────────────────────
if [ -n "$TARGET" ] && [ "${TARGET##*.}" = "AppImage" ]; then
  fuse_ok=0
  if ldconfig -p 2>/dev/null | grep -q "libfuse2\.so\.2\|libfuse\.so\.2"; then fuse_ok=1; fi
  if [ -x /bin/fusermount ] || [ -x /bin/fusermount3 ] || [ -x /usr/bin/fusermount ] || [ -x /usr/bin/fusermount3 ]; then fuse_ok=1; fi
  if [ "$fuse_ok" = 1 ]; then
    add "$OK" "FUSE2 presente — o AppImage abre com duplo-clique."
  else
    problems=$((problems + 1))
    add "$FAIL" "FUSE2 AUSENTE — é por isso que o AppImage 'não abre' nesta distro."
    add "  " "Opção A (agora, sem instalar): ./$(basename "$TARGET") --appimage-extract-and-run"
    add "  " "Opção B (definitiva): $(pkg_cmd fuse2)"
    add "  " "Opção C (sem FUSE): use o pacote portátil TurnRush-*-portable.tar.gz (extrair e rodar ./turnrush)"
  fi
fi

# ── 2. Sessão gráfica ──────────────────────────────────────────────────
if [ -n "${WAYLAND_DISPLAY:-}" ] || [ -S "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/${WAYLAND_DISPLAY:-wayland-0}" ]; then
  add "$OK" "Sessão Wayland detectada (${WAYLAND_DISPLAY:-wayland-0})."
elif [ -n "${DISPLAY:-}" ]; then
  add "$OK" "Sessão X11 detectada (DISPLAY=${DISPLAY})."
else
  problems=$((problems + 1))
  add "$FAIL" "Nenhuma sessão gráfica (nem WAYLAND_DISPLAY nem DISPLAY). Rode o jogo de dentro do desktop."
fi

# ── 3. GPU / Vulkan ────────────────────────────────────────────────────
gpu_ok=0
if ls /dev/dri/card* >/dev/null 2>&1 || ls /dev/dri/renderD* >/dev/null 2>&1; then gpu_ok=1; fi
icds=""
for d in /usr/share/vulkan/icd.d /etc/vulkan/icd.d /usr/local/share/vulkan/icd.d; do
  [ -d "$d" ] && icds="$icds $(ls "$d" 2>/dev/null | grep -c json || true)"
done
if [ "$gpu_ok" = 1 ] && [ -n "$(printf '%s' "$icds" | tr -d ' ')" ]; then
  add "$OK" "GPU acessível (/dev/dri) e drivers Vulkan instalados."
else
  problems=$((problems + 1))
  add "$WARN" "GPU/Vulkan incompleto (sem /dev/dri ou sem ICDs Vulkan) — o jogo pode não renderizar."
  add "  " "AMD/Intel: $(pkg_cmd vulkan-icd-loader vulkan-radeon vulkan-intel | sed 's/vulkan-icd-loader vulkan-radeon vulkan-intel/vulkan-radeon (ou vulkan-intel)/')"
  add "  " "Máquina virtual/sem GPU 3D: $(pkg_cmd vulkan-swrast mesa | sed 's/vulkan-swrast mesa/vulkan-swrast/')" 
fi

# ── 4. Bibliotecas do binário (ldd) ────────────────────────────────────
BIN=""
if [ -n "$TARGET" ]; then
  case "$TARGET" in
    *.AppImage)
      EXTRACT="$(mktemp -d)"
      ABS_TARGET="$(readlink -f "$TARGET")"
      # o runtime extrai ./squashfs-root no CWD — extrair DENTRO do tempdir
      if (cd "$EXTRACT" && "$ABS_TARGET" --appimage-extract >/dev/null 2>&1); then
        BIN="$EXTRACT/squashfs-root/usr/bin/turnrush-game"
        [ -d "$EXTRACT/squashfs-root/usr/lib" ] && export LD_LIBRARY_PATH="$EXTRACT/squashfs-root/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
        APPIMAGE_HINT="(extraído temporariamente em $EXTRACT — pode apagar depois)"
      else
        add "$FAIL" "Não consegui extrair o AppImage (--appimage-extract falhou)."
        problems=$((problems + 1))
      fi
      ;;
    *) BIN="$TARGET" ;;
  esac
fi

if [ -n "$BIN" ] && [ -f "$BIN" ]; then
  missing="$(ldd "$BIN" 2>/dev/null | grep 'not found' || true)"
  if [ -z "$missing" ]; then
    add "$OK" "Todas as bibliotecas do binário resolvem neste sistema."
  else
    problems=$((problems + 1))
    add "$FAIL" "Bibliotecas faltando no sistema:"
    echo "$missing" | while IFS= read -r line; do
      lib="$(printf '%s' "$line" | awk '{print $1}')"
      pkgs="$(pkg_for "$lib")"
      add "  " "  • $lib → $(pkg_cmd $pkgs)"
    done
  fi
fi

echo "$summary"
echo ""
if [ -n "$APPIMAGE_HINT" ]; then echo "Nota: $APPIMAGE_HINT"; echo ""; fi
if [ "$problems" -gt 0 ]; then
  echo "Resultado: $problems problema(s). Aplique as 'Opções' acima e rode o doctor de novo."
  exit 1
fi
echo "Resultado: tudo OK — o TurnRush deve abrir. Bora jogar! 🎮"
