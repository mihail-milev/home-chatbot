#!/bin/bash
set -o errexit

container=$(buildah from --arch aarch64 docker.io/arm64v8/fedora:35)
mountpoint=$(buildah mount $container)

cp target/aarch64-unknown-linux-gnu/release/home-chatbot $mountpoint/
chown 1000:1000 $mountpoint/home-chatbot
chmod a+x $mountpoint/home-chatbot

buildah config --user 1000:1000 $container
buildah config --entrypoint /home-chatbot $container

buildah commit --format docker $container home-chatbot

buildah unmount $container