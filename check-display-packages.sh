#!/bin/bash
# Check if X11 or Wayland packages are installed on Arch Linux

echo "=== Checking Display Server Packages ==="
echo ""

# Check for X11 packages
echo "X11 Packages:"
X11_INSTALLED=false
if pacman -Q xorg-server &>/dev/null; then
    echo "  ✓ xorg-server: $(pacman -Q xorg-server | awk '{print $2}')"
    X11_INSTALLED=true
else
    echo "  ✗ xorg-server: NOT INSTALLED"
fi

if pacman -Q xorg-xinit &>/dev/null; then
    echo "  ✓ xorg-xinit: $(pacman -Q xorg-xinit | awk '{print $2}')"
else
    echo "  ✗ xorg-xinit: NOT INSTALLED"
fi

if pacman -Q libx11 &>/dev/null; then
    echo "  ✓ libx11: $(pacman -Q libx11 | awk '{print $2}')"
else
    echo "  ✗ libx11: NOT INSTALLED"
fi

echo ""
echo "Wayland Packages:"
WAYLAND_INSTALLED=false
if pacman -Q wayland &>/dev/null; then
    echo "  ✓ wayland: $(pacman -Q wayland | awk '{print $2}')"
    WAYLAND_INSTALLED=true
else
    echo "  ✗ wayland: NOT INSTALLED"
fi

if pacman -Q libwayland-client &>/dev/null; then
    echo "  ✓ libwayland-client: $(pacman -Q libwayland-client | awk '{print $2}')"
else
    echo "  ✗ libwayland-client: NOT INSTALLED"
fi

echo ""
echo "XWayland (X11 on Wayland):"
if pacman -Q xorg-xwayland &>/dev/null; then
    echo "  ✓ xorg-xwayland: $(pacman -Q xorg-xwayland | awk '{print $2}')"
else
    echo "  ✗ xorg-xwayland: NOT INSTALLED"
fi

echo ""
echo "Display/Desktop Environment:"
# Check for common display managers
for dm in gdm sddm lightdm ly; do
    if pacman -Q $dm &>/dev/null; then
        echo "  ✓ Display Manager: $dm ($(pacman -Q $dm | awk '{print $2}'))"
    fi
done

# Check for common compositors/WMs
for wm in sway hyprland wayfire weston gnome-shell kwin plasma-workspace xfce4-session i3 bspwm; do
    if pacman -Q $wm &>/dev/null; then
        echo "  ✓ Window Manager/Compositor: $wm"
    fi
done

echo ""
echo "=== Summary ==="
if [ "$X11_INSTALLED" = true ] && [ "$WAYLAND_INSTALLED" = true ]; then
    echo "Both X11 and Wayland support installed"
    exit 0
elif [ "$X11_INSTALLED" = true ]; then
    echo "Only X11 support installed"
    exit 0
elif [ "$WAYLAND_INSTALLED" = true ]; then
    echo "Only Wayland support installed"
    exit 1
else
    echo "⚠ WARNING: No display server packages detected!"
    echo "This system may be headless or running in console mode."
    echo ""
    echo "To install X11: sudo pacman -S xorg-server xorg-xinit"
    echo "To install Wayland: sudo pacman -S wayland"
    exit 2
fi
