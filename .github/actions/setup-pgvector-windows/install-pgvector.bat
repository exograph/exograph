REM Accept parameters with defaults
set "PG_VERSION=%1"
set "PGVECTOR_VERSION=%2"
set "CACHE_HIT=%3"
if "%PG_VERSION%"=="" set "PG_VERSION=14"
if "%PGVECTOR_VERSION%"=="" set "PGVECTOR_VERSION=0.6.2"

set "PGROOT=C:\Program Files\PostgreSQL\%PG_VERSION%"

echo PostgreSQL version: %PG_VERSION%
echo pgvector version: %PGVECTOR_VERSION%
echo Cache hit: %CACHE_HIT%
echo PGROOT: %PGROOT%

REM Check if pgvector DLL exists
if exist "%PGROOT%\lib\pgvector.dll" (
    echo pgvector DLL found at %PGROOT%\lib\pgvector.dll
    if "%CACHE_HIT%"=="true" (
        echo Cache was restored, verifying installation...
        goto :verify_install
    ) else (
        echo DLL exists but cache was not hit, proceeding with full installation...
    )
) else (
    echo pgvector DLL not found at %PGROOT%\lib\pgvector.dll
)

echo pgvector not found, compiling from source...
cd %RUNNER_TEMP%
git clone --branch v%PGVECTOR_VERSION% https://github.com/pgvector/pgvector.git
cd pgvector

call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
nmake /NOLOGO /F Makefile.win
nmake /NOLOGO /F Makefile.win install

:verify_install
REM Verify the installation files exist
echo.
echo Verifying pgvector installation...
echo Checking for files in %PGROOT%

if not exist "%PGROOT%\lib\pgvector.dll" (
    echo ERROR: pgvector.dll not found at %PGROOT%\lib\pgvector.dll
    dir "%PGROOT%\lib" 2>nul | findstr /i vector
    exit /b 1
)
echo [OK] Found pgvector.dll

if not exist "%PGROOT%\share\extension\vector.control" (
    echo ERROR: vector.control not found at %PGROOT%\share\extension\vector.control
    dir "%PGROOT%\share\extension" 2>nul | findstr /i vector
    exit /b 1
)
echo [OK] Found vector.control

REM List all vector-related SQL files
echo.
echo Vector SQL files found:
dir "%PGROOT%\share\extension\vector*.sql" 2>nul

echo.
echo pgvector installation verified successfully!