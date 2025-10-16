#!/bin/sh
# Modified to install Exograph, original license below.
# Copyright 2019 the Deno authors. All rights reserved. MIT license.

set -e

if ! command -v unzip >/dev/null; then
	echo "Error: unzip is required to install Exograph." 1>&2
	exit 1
fi

if [ "$OS" = "Windows_NT" ]; then
	target="x86_64-pc-windows-msvc"
else
	case $(uname -sm) in
	"Darwin x86_64")
		echo "Error: Intel Macs (x86_64) are no longer supported." 1>&2
		echo "Exograph now requires Apple Silicon (but you can build it yourself from sources)." 1>&2
		exit 1
		;;
	"Darwin arm64") target="aarch64-apple-darwin" ;;
	"Linux aarch64")
		echo "Error: Official Exograph builds for Linux aarch64 are not yet available." 1>&2
		exit 1
		;;
	*) target="x86_64-unknown-linux-gnu" ;;
	esac
fi

if [ $# -eq 0 ]; then
	exograph_uri="https://github.com/exograph/exograph/releases/latest/download/exograph-${target}.zip"
else
	exograph_uri="https://github.com/exograph/exograph/releases/download/${1}/exograph-${target}.zip"
fi

exograph_install="${EXOGRAPH_INSTALL:-$HOME/.exograph}"
bin_dir="$exograph_install/bin"
exe="$bin_dir/exo"

if [ ! -d "$bin_dir" ]; then
	mkdir -p "$bin_dir"
fi

curl --fail --location --progress-bar --output "$exe.zip" "$exograph_uri"
unzip -d "$bin_dir" -o "$exe.zip"
chmod +x "$bin_dir"/exo*
rm "$exe.zip"

echo "Exograph was installed successfully to $exe"
if command -v exo >/dev/null; then
	echo "Run 'exo --help' to get started"
else
	case $SHELL in
	/bin/zsh) shell_profile=".zshrc" ;;
	*) shell_profile=".bashrc" ;;
	esac
	echo "Manually add the directory to your \$HOME/$shell_profile (or similar)"
	echo "  export PATH=\"$bin_dir:\$PATH\""
	echo "Run '$exe --help' to get started"
fi
echo
echo "Stuck? File an issue at https://github.com/exograph/exograph"