@echo off
REM attool auto-update installer for Windows.
REM Args: %1 = new exe, %2 = current exe, optional %3/%4 = PSD sidecar paths
timeout /t 2 /nobreak > nul
if exist "%~3" goto retry_sidecar
goto retry
:retry_sidecar
copy /y "%~3" "%~4"
if errorlevel 1 (
  timeout /t 1 /nobreak > nul
  goto retry_sidecar
)
:retry
copy /y "%~1" "%~2"
if errorlevel 1 (
  timeout /t 1 /nobreak > nul
  goto retry
)
start "" "%~2"
