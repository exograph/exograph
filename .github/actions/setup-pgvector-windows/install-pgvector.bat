@echo off
setlocal enableextensions

REM Accept parameters with defaults (strip quotes using ~)
set "PG_VERSION=%~1"
set "PGVECTOR_VERSION=%~2"
if "%PG_VERSION%"=="" set "PG_VERSION=18"
if "%PGVECTOR_VERSION%"=="" set "PGVECTOR_VERSION=0.8.1"

set "PGROOT=C:\Program Files\PostgreSQL\%PG_VERSION%"

echo PostgreSQL version: %PG_VERSION%
echo pgvector version: %PGVECTOR_VERSION%
echo PGROOT: %PGROOT%

REM Verify PostgreSQL installation exists
if not exist "%PGROOT%\bin\pg_config.exe" (
    echo ERROR: PostgreSQL not found at %PGROOT%
    exit /b 1
)

echo Cloning pgvector repository...
cd %RUNNER_TEMP%
git clone --branch v%PGVECTOR_VERSION% https://github.com/pgvector/pgvector.git
if errorlevel 1 (
    echo ERROR: Failed to clone pgvector repository
    exit /b 1
)

cd pgvector

echo Setting up Visual Studio environment...
call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
if errorlevel 1 (
    echo ERROR: Failed to set up Visual Studio environment
    exit /b 1
)

REM Re-set PGROOT after vcvars64.bat (it may clear env vars)
set "PGROOT=C:\Program Files\PostgreSQL\%PG_VERSION%"
echo PGROOT after vcvars: %PGROOT%

echo Compiling pgvector...
nmake /NOLOGO /F Makefile.win
if errorlevel 1 (
    echo ERROR: Failed to compile pgvector
    exit /b 1
)

echo Installing pgvector...
nmake /NOLOGO /F Makefile.win install
if errorlevel 1 (
    echo ERROR: Failed to install pgvector
    exit /b 1
)

REM Verify installation
if not exist "%PGROOT%\lib\vector.dll" (
    echo ERROR: vector.dll not found after installation
    exit /b 1
)

echo pgvector installation completed successfully!
echo Installed files:
dir "%PGROOT%\lib\vector.dll"
dir "%PGROOT%\share\extension\vector*"
