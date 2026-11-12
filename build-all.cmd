@echo off
rem build-all.cmd: what CI makes, made on this computer (AGENTS.md, section 4).
rem
rem   build-all.cmd [check]
rem
rem Without options: the release build of the workspace (the demo, the runtime, the template of a
rem native plugin), the plugin of stable Rust built apart (examples\ww-grey), the plugin in C
rem where a compiler of C is found, and the folder of the demo with its plugins in dist\ww-demo,
rem zipped as dist\ww-demo-<version>.zip with the folder inside.
rem
rem   check   first the checks of a commit: the format of the workspace and of examples\ww-grey,
rem           clippy of the workspace, of W# for the browser (the target wasm32-unknown-unknown,
rem           which rustup adds) and of examples\ww-grey, and the tests of the workspace
rem
rem The tools it takes, each from a variable or else from the PATH:
rem   WW_CARGO   cargo, for example a script that runs it on half of the processor
rem   WW_PYTHON  Python 3 (else py -3, else python): tools\imports.py names the std of the demo
rem   WW_RUSTC   rustc, whose sysroot holds that std-<hash>.dll
rem   WW_CC      a compiler of C for the example in C: cl of Visual Studio is found by vswhere,
rem              else gcc or clang of the PATH; without one the example is left out
rem Windows 10 and 11 carry the rest: tar.exe makes the archive.

setlocal EnableExtensions
cd /d "%~dp0"

rem Started by cmd /c (a double click, PowerShell), its window would close at the end with the
rem result: it waits for a key then.
set "WW_PAUSE="
setlocal EnableDelayedExpansion
set "WW_LINE=!CMDCMDLINE!"
if /i not "!WW_LINE:%~nx0=!"=="!WW_LINE!" (endlocal & set "WW_PAUSE=1") else endlocal

set "DO_CHECK="
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
:args
if "%~1"=="" goto args_done
if "%~1"=="/?" goto help
if /i "%~1"=="help" goto help
if /i "%~1"=="check" (set "DO_CHECK=1") else goto usage
shift
goto args
:args_done

set "DIST=%CD%\dist"
set "OUT=%DIST%\ww-demo"
set "TAR=%SystemRoot%\System32\tar.exe"
set "CARGO_CMD=cargo"
if defined WW_CARGO set "CARGO_CMD=%WW_CARGO%"
call "%CARGO_CMD%" --version
if errorlevel 1 (
  echo cargo does not start: install Rust from https://rustup.rs or set WW_CARGO.
  goto failed
)
set "PY=%WW_PYTHON%"
if not defined PY (
  where py >nul 2>&1 && set "PY=py -3"
)
if not defined PY set "PY=python"
call %PY% --version
if errorlevel 1 (
  echo Python 3 does not start: install it from https://www.python.org or set WW_PYTHON.
  goto failed
)
rem rustc names the std of the demo: its sysroot holds std-<hash>.dll, its host the kind of
rem toolchain.
set "RUSTC_CMD=rustc"
if defined WW_RUSTC set "RUSTC_CMD=%WW_RUSTC%"
set "RUSTC_VERSION="
for /f "delims=" %%v in ('call "%RUSTC_CMD%" -V 2^>nul') do set "RUSTC_VERSION=%%v"
if not defined RUSTC_VERSION (
  echo rustc does not start: install Rust from https://rustup.rs or set WW_RUSTC.
  goto failed
)
set "SYSROOT="
for /f "delims=" %%s in ('call "%RUSTC_CMD%" --print sysroot') do set "SYSROOT=%%s"
set "RHOST="
for /f "tokens=2" %%h in ('call "%RUSTC_CMD%" -vV ^| findstr /b "host:"') do set "RHOST=%%h"
set "VERSION="
for /f "tokens=3" %%a in ('findstr /b /c:"version = " Cargo.toml') do if not defined VERSION set "VERSION=%%~a"
echo W# %VERSION%; %RUSTC_VERSION% (%RHOST%)
if not exist "%DIST%" mkdir "%DIST%"

rem ---- The checks of a commit, the tests -------------------------------------------------------

if not defined DO_CHECK goto checks_done
call :step "Format, clippy and the tests of the workspace"
call "%CARGO_CMD%" fmt --all --check
if errorlevel 1 goto failed
call :grey fmt --check || goto failed
call "%CARGO_CMD%" clippy --workspace --all-targets -- -D warnings
if errorlevel 1 goto failed
rem W# for the browser, as the Linux job of CI checks it.
call "%CARGO_CMD%" clippy -p ww --no-default-features --features glow --target wasm32-unknown-unknown -- -D warnings
if errorlevel 1 goto failed
call "%CARGO_CMD%" test --workspace
if errorlevel 1 goto failed
call :grey clippy --all-targets -- -D warnings || goto failed
:checks_done

rem ---- The release builds ----------------------------------------------------------------------

call :step "Release build of the workspace"
call "%CARGO_CMD%" build --release --workspace
if errorlevel 1 goto failed
call :step "Release build of the plugin of stable Rust"
call :grey build --release || goto failed

call :step "The folder of the demo"
set "BIN=%CD%\target\release"
call :fresh "%OUT%" || goto failed
mkdir "%OUT%\plugins"
mkdir "%OUT%\samples"
copy /y "%BIN%\ww-demo.exe" "%OUT%\" >nul || goto failed
copy /y "%BIN%\wwrt.dll" "%OUT%\" >nul || goto failed
copy /y "%BIN%\ww_hello.dll" "%OUT%\plugins\" >nul || goto failed
copy /y examples\ww-grey\target\release\ww_grey.dll "%OUT%\plugins\" >nul || goto failed
copy /y examples\ww-mandelbrot\seahorse.mandel "%OUT%\samples\" >nul || goto failed
copy /y examples\ww-demo\samples\demo.wwbundle "%OUT%\samples\" >nul || goto failed
copy /y examples\ww-demo\package\README.md "%OUT%\README.md" >nul || goto failed
copy /y LICENSE "%OUT%\LICENSE" >nul || goto failed
rem The std-<hash>.dll ww-demo.exe imports, from the sysroot of rustc.
set "STD_NAME="
for /f "delims=" %%n in ('call %PY% tools\imports.py --std "%BIN%\ww-demo.exe"') do set "STD_NAME=%%n"
if not defined STD_NAME goto failed
call :sysroot_dll "%STD_NAME%" || goto failed
rem The toolchain gnu links the unwinder of MinGW: its two DLLs go along.
if "%RHOST:gnu=%"=="%RHOST%" goto gnu_done
call :sysroot_dll libgcc_s_seh-1.dll || goto failed
call :sysroot_dll libwinpthread-1.dll || goto failed
:gnu_done

rem ---- The plugin in C -------------------------------------------------------------------------

call :step "The plugin in C"
set "C_SOURCE=examples\ww-mandelbrot\mandelbrot.c"
set "C_DLL=%OUT%\plugins\ww_mandelbrot.dll"
if defined WW_CC goto c_named
set "VCVARS="
if exist "%VSWHERE%" for /f "usebackq delims=" %%v in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VCVARS=%%v\VC\Auxiliary\Build\vcvars64.bat"
if not defined VCVARS goto c_gcc
if not exist "%VCVARS%" goto c_gcc
rem /MT links the C runtime in, so the DLL needs no DLL of the Visual C++ runtime.
set "C_OBJ=%TEMP%\ww-mandelbrot-%RANDOM%"
mkdir "%C_OBJ%"
cmd /c ""%VCVARS%" >nul && cl /nologo /LD /O2 /MT /W3 /std:c11 /I stable "%C_SOURCE%" /Fo"%C_OBJ%\\" /Fe"%C_DLL%"" || goto failed
rmdir /s /q "%C_OBJ%"
del /q "%OUT%\plugins\ww_mandelbrot.lib" "%OUT%\plugins\ww_mandelbrot.exp" 2>nul
goto c_done
:c_gcc
where gcc >nul 2>&1 && (set "WW_CC=gcc" & goto c_named)
where clang >nul 2>&1 && (set "WW_CC=clang" & goto c_named)
echo The plugin in C is left out: no compiler of C here (Visual Studio, gcc or clang, or WW_CC).
goto c_done
:c_named
rem gcc of MinGW links its own runtime in with -static-libgcc.
set "C_FLAGS="
if /i "%WW_CC%"=="gcc" set "C_FLAGS=-static-libgcc"
call "%WW_CC%" -shared -O2 -std=c11 %C_FLAGS% -I stable "%C_SOURCE%" -o "%C_DLL%"
if errorlevel 1 goto failed
:c_done

call :step "The archive"
call %PY% tools\imports.py "%OUT%\ww-demo.exe" "%OUT%\wwrt.dll"
for %%f in ("%OUT%\plugins\*.dll") do call %PY% tools\imports.py "%%f"
if exist "%DIST%\ww-demo-%VERSION%.zip" del /q "%DIST%\ww-demo-%VERSION%.zip"
"%TAR%" -a -c -f "%DIST%\ww-demo-%VERSION%.zip" -C "%DIST%" ww-demo
if errorlevel 1 goto failed

echo.
echo Done at %TIME% in %DIST%
for %%f in ("%DIST%\*.zip") do echo   %%~nxf  %%~zf bytes
if defined WW_PAUSE pause
exit /b 0

:failed
echo.
echo build-all.cmd stopped at %TIME%: the step above failed.
if defined WW_PAUSE pause
exit /b 1

:usage
echo Unknown option: %1
echo.
call :options
if defined WW_PAUSE pause
exit /b 2

:help
call :options
if defined WW_PAUSE pause
exit /b 0

:options
echo build-all.cmd [check]
echo.
echo Without options: the release builds of the workspace, of the plugin of stable Rust and of the
echo plugin in C, and the folder of the demo with its plugins, zipped as dist\ww-demo-^<version^>.zip.
echo.
echo   check   first the format, clippy and the tests of the workspace, clippy of W# for the
echo           browser, the format and clippy of examples\ww-grey
echo.
echo WW_CARGO, WW_PYTHON, WW_RUSTC and WW_CC name the tools when the PATH has none of them.
exit /b 0

rem ---- Subroutines -----------------------------------------------------------------------------

:step
echo.
echo ==== %~1
exit /b 0

rem cargo in examples\ww-grey, a workspace of its own: RUSTFLAGS stands for the flags of the
rem native family in .cargo\config.toml (std linked dynamically), so the plugin links std in as a
rem plugin of others does; for msvc the C runtime too.
:grey
pushd examples\ww-grey
setlocal
set "RUSTFLAGS=-C debuginfo=0"
if "%RHOST:msvc=%" neq "%RHOST%" set "RUSTFLAGS=-C target-feature=+crt-static"
call "%CARGO_CMD%" %*
set "GREY_RESULT=%ERRORLEVEL%"
endlocal & set "GREY_RESULT=%GREY_RESULT%"
popd
exit /b %GREY_RESULT%

rem An empty folder: the one of an earlier run removed first.
:fresh
if exist "%~1" rmdir /s /q "%~1"
mkdir "%~1"
exit /b %ERRORLEVEL%

rem A file of the sysroot of rustc (a name or a pattern) into the folder of the demo.
:sysroot_dll
set "FOUND="
for /f "delims=" %%f in ('dir /s /b "%SYSROOT%\%~1" 2^>nul') do if not defined FOUND set "FOUND=%%f"
if not defined FOUND (
  echo %~1 is not in the sysroot of rustc.
  exit /b 1
)
copy /y "%FOUND%" "%OUT%\" >nul
exit /b %ERRORLEVEL%
