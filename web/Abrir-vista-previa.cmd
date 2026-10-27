@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Abrir-vista-previa.ps1"
if errorlevel 1 pause

