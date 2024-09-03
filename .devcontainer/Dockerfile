ARG VARIANT="bullseye"
FROM mcr.microsoft.com/vscode/devcontainers/rust:1-${VARIANT}

# Install the necessary tools
RUN apt-get update && apt-get install -y \
    locales \
    wget \
    unzip \
    zip \
    jq \
    curl \
    passwd \
    sudo \
    musl-dev \
    musl-tools \
    && apt-get clean 

# Configure sudo access and locale
ARG USERNAME=root
RUN echo "$USERNAME ALL=(root) NOPASSWD:ALL" > /etc/sudoers.d/$USERNAME \
    && chmod 0440 /etc/sudoers.d/$USERNAME\
    && echo "en_US.UTF-8 UTF-8" > /etc/locale.gen \
    && locale-gen en_US.UTF-8 \
    && update-locale LANG=en_US.UTF-8 \
    && dpkg-reconfigure --frontend=noninteractive locales

#Switch to the user
USER ${USERNAME}
ENV PATH="${HOME}/.cargo/bin:/usr/local/bin:${PATH}"
ENV LANG=en_US.UTF-8
ENV LANGUAGE=en_US:en
ENV LC_ALL=en_US.UTF-8

#Install other rust tools
RUN cargo install cargo-lambda
RUN rustup target add  x86_64-unknown-linux-musl
