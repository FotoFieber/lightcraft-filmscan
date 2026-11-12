#!/bin/sh
# Launcher of LightCraft Film Scan, installed as /usr/bin/lightcraft-filmscan (the app) and
# /usr/bin/lightcraft-filmscan-cli (the command-line tool). It runs beside an official LightCraft:
# its own settings folder, its own default library and its own app id (window, menu entry, icon).
# Sharing them would let the official LightCraft open photos edited with Convert Negative, which it
# can't show and could save back without the film settings. Set the variables yourself to choose.
: "${LIGHTCRAFT_CONFIG_DIR:=${XDG_CONFIG_HOME:-$HOME/.config}/lightcraft-filmscan}"
: "${LIGHTCRAFT_LIBRARY:=$HOME/Pictures/LightCraft Film Scan Library}"
: "${LIGHTCRAFT_APP_ID:=ch.fotofieber.LightCraftFilmScan}"
export LIGHTCRAFT_CONFIG_DIR LIGHTCRAFT_LIBRARY LIGHTCRAFT_APP_ID
case "$(basename "$0")" in
  *-cli) exec /usr/lib/lightcraft-filmscan/lightcraft-cli "$@" ;;
  *) exec /usr/lib/lightcraft-filmscan/lightcraft "$@" ;;
esac
