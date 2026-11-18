# LightCraft Film Scan

This is **LightCraft Film Scan**, a modified LightCraft by FotoFieber. It is **not an official release** and is
not made or endorsed by the ArtCraft team. It is LightCraft (by the ArtCraft team and the LightCraft contributors,
https://github.com/storytold/lightcraft) plus **Convert Negative**: film negative and slide scans to positives,
**Zap!** (find the frame on a camera scan, straighten, crop and measure the film base in one step) and a **roll
mode** that measures several frames together.

It is a playground for exploring film development features before they may come to LightCraft itself.

**Use at your own risk.** This build comes without any warranty (see LICENSE-MIT and LICENSE-APACHE). The Windows
and macOS builds are not signed. Keep backups of your photos.

- The full documentation: https://github.com/FotoFieber/lightcraft-filmscan#readme
- Releases, and what was tested for each: https://github.com/FotoFieber/lightcraft-filmscan/releases
- Report problems with this build here, not upstream: https://github.com/FotoFieber/lightcraft-filmscan/issues

## Install and start

**Linux (.deb):** `sudo apt install ./lightcraft-filmscan-<ver>-linux-x86_64.deb`. The package installs beside an
official LightCraft. Start **LightCraft Film Scan** from the menu, or run `lightcraft-filmscan` (the command-line
tool is `lightcraft-filmscan-cli`).

**Windows:** unzip the file anywhere, then start `lightcraft-filmscan.exe` in the folder **LightCraft Film Scan**. It
is not signed, so SmartScreen asks first: click **More info ▸ Run anyway**.

**macOS:** unzip the file and drag **LightCraft Film Scan** to Applications. The app is not signed by Apple, so macOS
refuses to open it the first time. Open System Settings ▸ Privacy & Security and click **Open Anyway**, or run
`xattr -dr com.apple.quarantine "/Applications/LightCraft Film Scan.app"`.

## Settings and library

LightCraft Film Scan keeps its own settings and its own library, so the official LightCraft never opens photos with
film settings it doesn't know.

| System | Settings | Library |
|---|---|---|
| Linux | `~/.config/lightcraft-filmscan` | `~/Pictures/LightCraft Film Scan Library` |
| Windows | `%APPDATA%\LightCraft Film Scan` | `LightCraft Film Scan Library` in your Pictures folder |
| macOS | `~/Library/Application Support/LightCraft Film Scan` | `LightCraft Film Scan Library` in your Pictures folder |

On Linux, `LIGHTCRAFT_CONFIG_DIR` and `LIGHTCRAFT_LIBRARY` choose other folders.

## Licence

LightCraft is dual-licensed under MIT or Apache-2.0, at your option. Copyright (c) 2026 ArtCraft Team and the
LightCraft contributors. The required notices are in NOTICE. The font licences are in this package too.
