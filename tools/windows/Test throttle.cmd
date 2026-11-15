@echo off
rem Double-click to hold 50%% throttle on the virtual controller (build step 1).
set SECONDS=%~1
if "%SECONDS%"=="" set SECONDS=30
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" throttle %SECONDS%
pause
