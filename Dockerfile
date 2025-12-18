FROM archlinux:latest
LABEL contributor="shadowapex@gmail.com"

# Initialize pacman keyring and update system
RUN pacman-key --init && \
  pacman-key --populate archlinux && \
  echo -e "keyserver-options auto-key-retrieve" >> /etc/pacman.d/gnupg/gpg.conf && \
  pacman --noconfirm -Syyuu

# Copy pacman configuration
COPY rootfs/etc/pacman.conf /etc/pacman.conf

# Cannot check space in chroot
RUN sed -i '/CheckSpace/s/^/#/g' /etc/pacman.conf

# Sync package databases and install base build tools
RUN pacman --noconfirm -Syy && \
  pacman --noconfirm -S \
  arch-install-scripts \
  base-devel \
  btrfs-progs \
  fmt \
  git \
  pyalpm \
  python \
  python-build \
  python-flit-core \
  python-installer \
  python-hatchling \
  python-markdown-it-py \
  python-setuptools \
  python-wheel \
  rust \
  cargo \
  sudo \
  wget \
  xcb-util-wm \
  alsa-lib \
  dbus \
  systemd-libs \
  ffmpeg \
  clang \
  pkg-config

# Create build user with sudo access
RUN echo "%wheel ALL=(ALL) NOPASSWD: ALL" >> /etc/sudoers && \
  useradd build -G wheel -m

# Build and install pikaur (AUR helper)
RUN su - build -c "git clone https://aur.archlinux.org/pikaur.git /tmp/pikaur" && \
  su - build -c "cd /tmp/pikaur && makepkg -f" && \
  pacman --noconfirm -U /tmp/pikaur/pikaur-*.pkg.tar.zst && \
  rm -rf /tmp/pikaur

# Auto add PGP keys for users
RUN mkdir -p /etc/gnupg/ && echo -e "keyserver-options auto-key-retrieve" >> /etc/gnupg/gpg.conf

# Add a fake systemd-run script to workaround pikaur requirement.
RUN echo -e "#!/bin/bash\nif [[ \"$1\" == \"--version\" ]]; then echo 'fake 244 version'; fi\nmkdir -p /var/cache/pikaur\n" >> /usr/bin/systemd-run && \
  chmod +x /usr/bin/systemd-run

# substitute check with !check to avoid running software from AUR in the build machine
# also remove creation of debug packages.
RUN sed -i '/BUILDENV/s/check/!check/g' /etc/makepkg.conf && \
  sed -i '/OPTIONS/s/debug/!debug/g' /etc/makepkg.conf

COPY manifest /manifest
# Freeze packages and overwrite with overrides when needed
RUN source /manifest && \
  if [ -n "${ARCHIVE_DATE}" ]; then \
    echo "Server=https://archive.archlinux.org/repos/${ARCHIVE_DATE}/\$repo/os/\$arch" > /etc/pacman.d/mirrorlist; \
  fi && \
  pacman --noconfirm -Syyuu; \
  if [ -n "${PACKAGE_OVERRIDES}" ]; then \
    wget --directory-prefix=/tmp/extra_pkgs ${PACKAGE_OVERRIDES}; \
    pacman --noconfirm -U --overwrite '*' /tmp/extra_pkgs/*; \
    rm -rf /tmp/extra_pkgs; \
  fi

USER build
ENV BUILD_USER="build"
ENV GNUPGHOME="/etc/pacman.d/gnupg"
# Built image will be moved here. This should be a host mount to get the output.
ENV OUTPUT_DIR="/output"

WORKDIR /workdir
