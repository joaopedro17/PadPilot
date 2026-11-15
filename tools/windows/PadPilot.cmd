@echo off
rem Double-click to show the on-screen joystick.
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" overlay %*
if errorlevel 1 pause
