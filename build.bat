@echo off
setlocal
echo ============================================
echo  小火 xiaohuo 编译脚本（编译完自动清理 target）
echo ============================================

REM ---- 1. 确保 cargo 可用 ----
where cargo >nul 2>nul
if errorlevel 1 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
where cargo >nul 2>nul || (echo [错误] 找不到 cargo，请先安装 Rust：https://rustup.rs & exit /b 1)

REM ---- 2. 前端：若没有 dist 就现编 ----
if not exist "dist" (
    echo [信息] 未找到 dist，先用 npm 构建前端...
    where npm >nul 2>nul || (echo [错误] 找不到 npm，请先安装 Node.js & exit /b 1)
    call npm install
    call npm run build
)

REM ---- 3. 编译 Rust（仅产出 exe，不打包安装器）----
cd src-tauri
cargo build --release
if errorlevel 1 (echo [错误] 编译失败 & exit /b 1)
cd ..

REM ---- 4. 把 exe 拷到仓库根目录，方便直接启动 ----
copy /Y "src-tauri\target\release\xiaohuo.exe" "xiaohuo.exe"

REM ---- 5. 自动删除编译产生的多余文件（target 巨目录）----
rmdir /s /q "src-tauri\target"
echo.
echo [完成] xiaohuo.exe 已生成在仓库根目录，target 已清理。
echo 用法：复制 .env.example 为 .env，填入 ZHIPU_API_KEY，双击 xiaohuo.exe 启动。
pause
