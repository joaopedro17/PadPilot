@echo off
rem Safety test: right-click the joystick and hold full throttle. After 10 seconds
rem in joystick mode PadPilot freezes on purpose; within about 1 second the
rem watchdog must free the cursor and close PadPilot, and the car must stop.
"%~dp0..\..\target\x86_64-pc-windows-msvc\release\padpilot.exe" freeze %*
pause
