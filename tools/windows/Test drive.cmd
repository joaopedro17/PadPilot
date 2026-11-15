@echo off
rem Double-click to drive the virtual controller from the touchpad (build step 3).
set SECONDS=%~1
if "%SECONDS%"=="" set SECONDS=120
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" drive %SECONDS%
pause
