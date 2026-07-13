@echo off
REM attool auto-update installer for Windows.
REM Args: %1 = new exe path, %2 = current exe path to overwrite
timeout /t 2 /nobreak > nul
:retry
copy /y %1 %2
if errorlevel 1 (
  timeout /t 1 /nobreak > nul
  goto retry
)
start "" %2
