set "PGROOT=C:\Program Files\PostgreSQL\14"
cd $RUNNER_TEMP
git clone --branch v0.6.2 https://github.com/pgvector/pgvector.git
cd pgvector

call "C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Auxiliary\Build\vcvarsall.bat"
nmake /NOLOGO /F Makefile.win
nmake /NOLOGO /F Makefile.win install