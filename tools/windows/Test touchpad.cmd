@echo off
rem Double-click to run the touchpad test (build step 2).
set SECONDS=%~1
if "%SECONDS%"=="" set SECONDS=60
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" touchpad %SECONDS%
pause
