REM Accept parameters with defaults
set "PG_VERSION=%1"
set "PGVECTOR_VERSION=%2"
if "%PG_VERSION%"=="" set "PG_VERSION=14"
if "%PGVECTOR_VERSION%"=="" set "PGVECTOR_VERSION=0.6.2"

set "PGROOT=C:\Program Files\PostgreSQL\%PG_VERSION%"

REM Check if pgvector is already installed (cache hit)
if exist "%PGROOT%\lib\pgvector.dll" (
    echo pgvector already installed, skipping compilation
    exit /b 0
)

echo pgvector not found, compiling from source...
cd %RUNNER_TEMP%
git clone --branch v%PGVECTOR_VERSION% https://github.com/pgvector/pgvector.git
cd pgvector

call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
nmake /NOLOGO /F Makefile.win
nmake /NOLOGO /F Makefile.win install