#!/bin/sh
# Stabile App bauen und nach /Applications installieren (eigene Daten unter
# ~/Library/Application Support/de.printmanager.app). Nur auf Wunsch des Nutzers ausführen.
set -e
cd "$(dirname "$0")/.."
npx tauri build --bundles app
osascript -e 'quit app id "de.printmanager.app"' 2>/dev/null || true
sleep 1
rm -rf /Applications/Printmanager.app
ditto src-tauri/target/release/bundle/macos/Printmanager.app /Applications/Printmanager.app
open -a /Applications/Printmanager.app
echo "Installiert: /Applications/Printmanager.app ($(git describe --tags --always --dirty))"
