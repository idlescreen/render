# Status: placeholder — no published image yet.
#
# A full build requires the sibling `idle/` workspace as a path dependency
# (`idle-runner`, `idle-api`, `crates/wayland-idle`, `crates/wayland-present`,
# `crates/idle-dbus`, `crates/idle-ipc`, `crates/idle-upscaler`). The CI
# release matrix in `.github/workflows/ci.yml` builds those natively; the
# container pipeline is deferred until Sprint 05 H1 (see SPRINT.md).
#
# `unraid/render.xml` is the Community Apps template that points at this
# (currently non-existent) image.
#
FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev pkgconfig freetype-dev fontconfig-dev \
    dbus-dev wayland-dev libxkbcommon-dev openssl-dev
WORKDIR /src
COPY . /src/render
RUN echo "Image not yet published; see SPRINT.md Sprint 05 H1" > /build-note

FROM alpine:3.20
RUN apk add --no-cache ffmpeg ca-certificates
COPY --from=build /build-note /build-note
ENTRYPOINT ["/bin/sh"]
