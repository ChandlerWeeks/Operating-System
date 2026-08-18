FROM docker.io/alpine:latest

RUN apk update && apk add --no-cache \
    build-base \
    git \
    curl \
    make \
    python3 \
    python3-dev \
    py3-sphinx \
    ninja \
    cmake \
    glib-dev \
    pixman-dev \
    pkgconf \
    libpng-dev \
    libjpeg-turbo-dev \
    sdl2-dev \
    libcap-ng-dev \
    liburing-dev \
    vim \
    llvm \
    lldb \
    nano \
    # Additional deps needed for QEMU build on Alpine
    linux-headers \
    flex \
    bison \
    perl \
    zlib-dev \
    bash \
    socat

############################################################
##
##    Rust Programming Language
##
############################################################
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && /usr/local/cargo/bin/rustup toolchain install stable \
    && /usr/local/cargo/bin/rustup target add riscv64gc-unknown-none-elf \
    && /usr/local/cargo/bin/rustup component add rust-analyzer \
    && /usr/local/cargo/bin/rustup component add rust-src \
    && rm -rf /usr/local/rustup/toolchains/stable-*-unknown-linux-musl/share/doc \
    && rm -rf /usr/local/cargo/registry

# Add Rust binaries to PATH for all users
ENV PATH="/usr/local/cargo/bin:${PATH}"

############################################################
##
##    Quick Emulator
##
############################################################
ARG QEMU_VERSION=cosc562
ARG QEMU_URL=https://github.com/sgmarz/qemu.git

# Build QEMU from source
RUN git init /tmp/qemu \
    && cd /tmp/qemu \
    && git remote add origin ${QEMU_URL} \
    && git fetch --depth 1 origin ${QEMU_VERSION} \
    && git checkout FETCH_HEAD \
    && ./configure --target-list=riscv64-softmmu --prefix=/usr/local --disable-docs --disable-plugins --disable-werror --enable-vnc \
    && make -j$(nproc) \
    && make install \
    && cd / \
    && rm -rf /tmp/qemu

# Install FSTool
RUN /usr/local/cargo/bin/cargo install -q --git https://github.com/sgmarz/fstool.git --tag v0.1.1

# Set working directory
ARG USERNAME=cosc562
ARG USER_UID=9876
ARG USER_GID=$USER_UID

# Alpine uses addgroup/adduser (busybox) instead of groupadd/useradd
RUN addgroup -g 9870 cargo \
    && addgroup -g $USER_GID $USERNAME \
    && adduser -u $USER_UID -G $USERNAME -D -h /home/cosc562 $USERNAME \
    && addgroup $USERNAME cargo \
    && chown -R $USERNAME:$USERNAME /home/cosc562 \
    && chgrp -R cargo /usr/local/cargo \
    && chmod -R g+w /usr/local/cargo

RUN mkdir -p /home/cosc562/myos

WORKDIR /home/cosc562/myos