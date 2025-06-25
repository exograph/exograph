REM Accept parameters with defaults
set "PG_VERSION=%1"
set "PGVECTOR_VERSION=%2"
if "%PG_VERSION%"=="" set "PG_VERSION=14"
if "%PGVECTOR_VERSION%"=="" set "PGVECTOR_VERSION=0.6.2"

set "PGROOT=C:\Program Files\PostgreSQL\%PG_VERSION%"

echo PostgreSQL version: %PG_VERSION%
echo pgvector version: %PGVECTOR_VERSION%
echo PGROOT: %PGROOT%

echo Compiling pgvector from source...
cd %RUNNER_TEMP%
git clone --branch v%PGVECTOR_VERSION% https://github.com/pgvector/pgvector.git
cd pgvector

call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
nmake /NOLOGO /F Makefile.win
nmake /NOLOGO /F Makefile.win install

echo pgvector installation completed successfully!