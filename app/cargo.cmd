@echo off
set "CARGO_HOME=%USERPROFILE%\.cargo"
set "RUSTUP_HOME=%USERPROFILE%\.rustup"
if exist "%CARGO_HOME%\bin\cargo.exe" (
  "%CARGO_HOME%\bin\cargo.exe" %*
  exit /b %ERRORLEVEL%
)
where cargo.exe >nul 2>nul
if not errorlevel 1 (
  cargo.exe %*
  exit /b %ERRORLEVEL%
)
echo Cargo not found. Install Rust via rustup. 1>&2
exit /b 9009
