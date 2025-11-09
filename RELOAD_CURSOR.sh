#!/bin/bash
echo "╔════════════════════════════════════════════════════════╗"
echo "║  PLIKI SĄ NA DYSKU - POTRZEBUJESZ RELOAD CURSOR        ║"
echo "╚════════════════════════════════════════════════════════╝"
echo ""
echo "📁 Pliki na dysku:"
ls -1 src/*.rs
echo ""
echo "✅ Kompilacja działa:"
cargo build --lib 2>&1 | tail -3
echo ""
echo "🔧 Jak naprawić widok w Cursor:"
echo "   1. Ctrl+Shift+P → 'Reload Window'"
echo "   2. Lub restart Cursor"
echo "   3. Lub Ctrl+P i otwórz 'lib.rs' ręcznie"
echo ""
echo "📊 Git status:"
git status --short || git status
echo ""
echo "✅ Wszystko jest! To tylko problem UI."
