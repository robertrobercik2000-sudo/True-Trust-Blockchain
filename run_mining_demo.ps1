# TRUE_TRUST Mining Demo Runner
# This script runs the complete mining pipeline test

Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
Write-Host "║  TRUE_TRUST Mining Demo - Starting...                   ║" -ForegroundColor Cyan
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
Write-Host ""

Write-Host "⚠️  WARNING: RandomX initialization will take 30-60 seconds!" -ForegroundColor Yellow
Write-Host "    This is normal - generating 2GB dataset..." -ForegroundColor Yellow
Write-Host ""

$confirm = Read-Host "Press ENTER to start mining demo (or Ctrl+C to cancel)"

Write-Host ""
Write-Host "🚀 Launching mining demo..." -ForegroundColor Green
Write-Host ""

# Run the mining demo
& ".\target\release\examples\mining_demo.exe"

$exitCode = $LASTEXITCODE

Write-Host ""
if ($exitCode -eq 0) {
    Write-Host "✅ Mining demo completed successfully!" -ForegroundColor Green
} else {
    Write-Host "❌ Mining demo failed with exit code: $exitCode" -ForegroundColor Red
}

Write-Host ""
Write-Host "Press any key to exit..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")

