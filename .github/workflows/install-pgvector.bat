set "PGROOT=C:\Program Files\PostgreSQL\14"

REM Check if pgvector is already installed (cache hit)
if exist "%PGROOT%\lib\pgvector.dll" (
    echo pgvector already installed, skipping compilation
    exit /b 0
)

echo pgvector not found, compiling from source...
cd %RUNNER_TEMP%
git clone --branch v0.6.2 https://github.com/pgvector/pgvector.git
cd pgvector

call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Auxiliary\Build\vcvars64.bat"
nmake /NOLOGO /F Makefile.win
nmake /NOLOGO /F Makefile.win install