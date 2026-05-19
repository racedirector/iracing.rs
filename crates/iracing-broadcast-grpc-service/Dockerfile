# escape=`

ARG WINDOWS_VERSION=ltsc2022
ARG RUST_TOOLCHAIN=stable

FROM mcr.microsoft.com/windows/servercore:${WINDOWS_VERSION} AS builder

ARG RUST_TOOLCHAIN

SHELL ["powershell", "-NoLogo", "-NoProfile", "-Command", "$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue';"]

ENV CARGO_HOME=C:\cargo
ENV RUSTUP_HOME=C:\rustup
ENV PATH=C:\cargo\bin;C:\Windows\system32;C:\Windows;C:\Windows\System32\WindowsPowerShell\v1.0
ENV RUSTFLAGS="-C target-feature=+crt-static"

WORKDIR C:\src

RUN Invoke-WebRequest -UseBasicParsing -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe; `
    Start-Process -FilePath .\rustup-init.exe -ArgumentList @('-y', '--profile', 'minimal', '--default-host', 'x86_64-pc-windows-gnu', '--default-toolchain', $env:RUST_TOOLCHAIN) -NoNewWindow -Wait; `
    Remove-Item .\rustup-init.exe; `
    rustc --version; `
    cargo --version

COPY . .

RUN cargo build --release -p iracing-broadcast-grpc-service --bin iracing-broadcast-server --target x86_64-pc-windows-gnu

FROM mcr.microsoft.com/windows/servercore:${WINDOWS_VERSION} AS runtime

SHELL ["powershell", "-NoLogo", "-NoProfile", "-Command", "$ErrorActionPreference = 'Stop'; $ProgressPreference = 'SilentlyContinue';"]

WORKDIR C:\app

ENV BROADCAST_ADDR=0.0.0.0:50051
ENV RUST_LOG=info

COPY --from=builder C:\src\target\x86_64-pc-windows-gnu\release\iracing-broadcast-server.exe C:\app\iracing-broadcast-server.exe

EXPOSE 50051

ENTRYPOINT ["C:\\app\\iracing-broadcast-server.exe"]
