#!/bin/bash

# Error trapping from https://gist.github.com/oldratlee/902ad9a398affca37bfcfab64612e7d1
__error_trapper() {
  local parent_lineno="$1"
  local code="$2"
  local commands="$3"
  echo "error exit status $code, at file $0 on or near line $parent_lineno: $commands"
}
trap '__error_trapper "${LINENO}/${BASH_LINENO}" "$?" "$BASH_COMMAND"' ERR

set -euE -o pipefail
shopt -s failglob

# Cron's path tends to suck
export PATH=/usr/local/bin:/usr/local/sbin:/usr/bin:/usr/sbin:$HOME/bin:$HOME/.local/bin

selfdir="$(readlink -f "$(dirname "$0")")"
cd "$selfdir"

podman build -t telegram_images .

mkdir -p ~/.local/rust_docker_cargo-telegram-images
chcon -R -t container_file_t ~/.local/rust_docker_cargo-telegram-images

podman kill telegram-images || true
podman rm telegram-images || true

echo "cleaning old images"
rm -rf output/
mkdir output

echo "running file extractor"
# See https://github.com/xd009642/tarpaulin/issues/1087 for the seccomp thing
podman run --rm --name telegram-images --security-opt seccomp=~/src/neovim_rust/seccomp.json -w /root/src/telegram-images \
  -v ~/src:/root/src -v ~/.local/rust_docker_cargo-telegram-images:/root/.cargo \
  -v ~/config/dotfiles/nvim:/root/.config/nvim  -v ~/config/dotfiles/bashrc:/root/.bashrc \
  -v ~/config/dotfiles/bothrc:/root/.bothrc \
  -it telegram_images cargo run "$@"

echo "copying images"
rsync -av output/ ~/Dropbox/Pictures/ZF_Prep/Telegram_Files/

echo "telegram images run complete"
