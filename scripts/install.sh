#!/usr/bin/env bash
set -euo pipefail

APP="mew"
REPO_URL="${MEW_REPO_URL:-https://github.com/mahesh953-hub/mew}"
PREFIX_DIR="${MEW_INSTALL_DIR:-$HOME/.local/bin}"
SRC_DIR="${MEW_SRC_DIR:-$HOME/.cache/mew-src}"

say() {
  printf "%s\n" "$*"
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1
}

is_termux() {
  [ -n "${TERMUX_VERSION:-}" ] || echo "${PREFIX:-}" | grep -q "com.termux"
}

install_deps_termux() {
  say "mew: installing Termux dependencies"
  pkg update -y
  pkg install -y git curl rust clang make pkg-config openssl
}

install_deps_debian() {
  say "mew: installing Debian/Ubuntu dependencies"
  apt-get update
  apt-get install -y git curl build-essential pkg-config libssl-dev ca-certificates
}

install_rustup() {
  if need_cmd cargo; then
    return
  fi

  say "mew: installing rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1090
  . "$HOME/.cargo/env"
}

prepare_deps() {
  if is_termux; then
    install_deps_termux
  elif need_cmd apt-get && [ "$(id -u)" = "0" ]; then
    install_deps_debian
  elif need_cmd apt-get && need_cmd sudo; then
    sudo apt-get update
    sudo apt-get install -y git curl build-essential pkg-config libssl-dev ca-certificates
  fi

  if ! need_cmd curl; then
    say "mew: curl missing"
    exit 1
  fi

  if ! need_cmd git; then
    say "mew: git missing"
    exit 1
  fi

  install_rustup

  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
  fi
}

install_from_git() {
  mkdir -p "$PREFIX_DIR"

  if [ -d "$SRC_DIR/.git" ]; then
    say "mew: updating source"
    git -C "$SRC_DIR" pull --ff-only
  else
    say "mew: cloning source"
    rm -rf "$SRC_DIR"
    git clone "$REPO_URL" "$SRC_DIR"
  fi

  cd "$SRC_DIR"

  say "mew: building release binary"
  cargo build --release -p mew-cli

  cp target/release/mew "$PREFIX_DIR/mew"
  chmod +x "$PREFIX_DIR/mew"

  say "mew: installed to $PREFIX_DIR/mew"

  case ":$PATH:" in
    *":$PREFIX_DIR:"*) ;;
    *)
      say ""
      say "add this to your shell:"
      say "  export PATH=\"$PREFIX_DIR:\$PATH\""
      ;;
  esac

  say ""
  "$PREFIX_DIR/mew" --help
}

prepare_deps
install_from_git
