# Linux build and headless desktop test image (Ubuntu 24.04, GNOME Shell 46, KWin 5.27).
# docker build -t menew-dev scripts/dev
# docker run --rm -v "$PWD:/src" -v menew-target:/target -v menew-cargo:/root/.cargo/registry menew-dev cargo test --workspace
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git build-essential pkg-config \
    libgtk-4-dev libadwaita-1-dev libdbus-1-dev \
    gnome-shell gnome-shell-extensions mutter kwin-wayland kde-spectacle qtwayland5 wl-clipboard \
    xdg-desktop-portal xdg-desktop-portal-gtk sqlite3 imagemagick dbus-user-session dbus-x11 dbus-bin \
    libglib2.0-bin python3-gi gir1.2-gtk-4.0 python3-yaml procps psmisc tesseract-ocr \
    && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --component clippy
ENV PATH=/root/.cargo/bin:$PATH CARGO_TARGET_DIR=/target
WORKDIR /src
