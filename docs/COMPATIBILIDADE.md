# Compatibilidade de distros — "o jogo não abre, e agora?"

O AppImage e o pacote portátil do TurnRush são **autocontidos**: a stack
cliente gráfica (X11/xcb, Xcursor, Xrandr, Xi, Xinerama, xkbcommon,
xkbcommon-x11, wayland-client/cursor) viaja **dentro** do pacote. Do sistema
hospedeiro o jogo precisa apenas de:

1. **glibc** (qualquer distro 2024+);
2. **sessão gráfica** — Wayland nativo (labwc, sway, GNOME, KDE…) **ou** X11
   (inclusive via XWayland);
3. **drivers gráficos** (Vulkan de preferência; GL 3.3+ como fallback) —
   não podem ser embutidos: precisam casar com o kernel/GPU da máquina;
4. **FUSE2** — apenas para abrir o AppImage com duplo-clique (o portátil
   prescinde).

## As 3 formas de rodar

| Formato | Comando | Exige do host |
|---|---|---|
| **AppImage** | `chmod +x TurnRush-*.AppImage && ./TurnRush-*.AppImage` | FUSE2 + glibc + drivers |
| **AppImage sem FUSE** | `./TurnRush-*.AppImage --appimage-extract-and-run` | glibc + drivers |
| **Portátil** | extrair `TurnRush-*-portable.tar.gz` e rodar `./turnrush.AppDir/turnrush` | glibc + drivers (sem FUSE) |

Dúvida em qualquer distro? Baixe `turnrush-doctor.sh` da release e rode:

```bash
./turnrush-doctor.sh TurnRush-*.AppImage    # ou ./turnrush-game (portátil)
```

Ele diz em pt-BR o que falta e imprime o comando pronto para
pacman/apt/dnf/zypper.

## Por distro

### Archcraft Elite / Arch / CachyOS / Archcraft

A Elite (Wayland-only, labwc) roda o TurnRush **nativo em Wayland** — o
AppImage/portátil já trazem a stack X11/xkb embutida, então o debloat da
distro não atrapalha. Só falta o Vulkan (de série na Elite: RADV/NVK) e,
para duplo-clique do AppImage, o FUSE2:

```bash
sudo pacman -S --needed vulkan-icd-loader vulkan-radeon   # AMD (ou vulkan-intel)
sudo pacman -S --needed fuse2                             # só p/ duplo-clique do AppImage
```

Máquina virtual ou sem GPU 3D: `sudo pacman -S --needed vulkan-swrast`.

### Ubuntu / Debian / Mint / Pop

```bash
sudo apt install -y mesa-vulkan-drivers libvulkan1   # Vulkan (AMD/Intel)
sudo apt install -y libfuse2                          # só p/ duplo-clique do AppImage
```

VM/sem GPU: `sudo apt install -y mesa-vulkan-drivers` já inclui o lavapipe
(software). NVIDIA: `nvidia-vulkan-icd` (ou driver proprietário ≥ 550).

### Fedora

```bash
sudo dnf install -y mesa-vulkan-drivers vulkan-loader
sudo dnf install -y fuse                              # só p/ duplo-clique do AppImage
```

## O jogo não abriu — passo a passo

1. **Rode o doctor** (`./turnrush-doctor.sh TurnRush-*.AppImage`). Ele cobre
   os 4 requisitos acima com comandos prontos.
2. **"dlopen(): error loading libfuse.so.2"** ou duplo-clique que não faz
   nada → sua distro não tem FUSE2. Use
   `./TurnRush-*.AppImage --appimage-extract-and-run`, instale `fuse2`
   (Arch) / `libfuse2` (Ubuntu) ou use o **portátil**.
3. **"Failed to find Vulkan ICD"** / pânico citando adapter/Vulkan → instale
   o driver Vulkan da sua GPU (tabelas acima). Sem GPU 3D → `vulkan-swrast`
   (Arch) / `mesa-vulkan-drivers` (Ubuntu).
4. **Abriu e fechou na hora** → veja `~/TurnRush/logs/crash-*.log` (o jogo
   grava backtrace + últimas linhas de sessão) e abra a issue com o arquivo.
5. Nada resolveu? Abra a issue colando **a saída completa do doctor** — ela
   já contém tudo que precisamos.

## O que vai embutido × o que vem do host

| Embutido no AppImage/portátil | Sempre do host |
|---|---|
| libX11, libX11-xcb, libxcb + extensões (cursor, render, shape, xfixes, xkb, icccm, keysyms, randr, xinerama), libXau, libXdmcp | glibc (libc, libm, ld-linux, libgcc_s) |
| libXcursor, libXrandr, libXi, libXinerama | Drivers Vulkan (RADV/NVK/NVIDIA) |
| libxkbcommon, libxkbcommon-x11 | Drivers GL/EGL (fallback) |
| libwayland-client, libwayland-cursor | Compositor (labwc/wlroots/GNOME/KDE) |

**Por que glibc e drivers não são embutidos?** A glibc é parte do contrato
ABI do sistema (embuti-la quebra o carregador dinâmico) e os drivers gráficos
precisam corresponder exatamente ao kernel/GPU da máquina — empacotá-los
seria a receita do "funciona na minha, quebra na sua".
