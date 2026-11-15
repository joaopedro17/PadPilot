@echo off
rem Double-click to try the on-screen joystick. Joystick mode ends by itself after 10 seconds.
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" overlay 10
if errorlevel 1 pause
