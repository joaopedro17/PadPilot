@echo off
rem Double-click to use the on-screen joystick while recording input timing.
rem The CSV file goes to the logs folder; its path is shown when you close the window.
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" telemetry %*
pause
