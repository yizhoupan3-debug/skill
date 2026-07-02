<#
.SYNOPSIS
  一键安装 Skill Harness 到 Windows + Codex CLI
.DESCRIPTION
  自动完成：依赖检查 → 克隆仓库 → 全量编译 Rust 工具 → 安装 Codex 宿主投影 →
  提取二进制 → 彻底清理编译中间产物。
.PARAMETER RepoUrl
  框架仓库 Git URL（默认从远程 origin 自动推断）
.PARAMETER FrameworkRoot
  安装路径（默认 $env:USERPROFILE\Developer\skill）
.PARAMETER SkipDeps
  跳过依赖安装（已装好时使用）
.PARAMETER KeepRegistryCache
  保留 ~\.cargo\registry\cache 和 src（默认删除以节省空间，下次构建会重下）
#>

param(
  [string]$RepoUrl = "",
  [string]$FrameworkRoot = "$env:USERPROFILE\Developer\skill",
  [switch]$SkipDeps,
  [switch]$KeepRegistryCache
)

$ErrorActionPreference = "Stop"
$Host.UI.RawUI.ForegroundColor = "White"

# ─── helpers ──────────────────────────────────────────────────────────
function ColorLine($color, $msg) { Write-Host $msg -ForegroundColor $color }
function Step($num, $title, $scriptBlock) {
  ColorLine Cyan "`n=== $num/7 $title ===`n"
  & $scriptBlock
  if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "步骤 $num 失败 (exit=$LASTEXITCODE)" }
}

# ─── 0. 项目名映射 ───────────────────────────────────────────────────
# binary_name → workspace_crate 对应关系，仅用于安装到 PATH
$BINARIES = @(
  "router-rs-cli",
  "mcp-codegraph",
  "mcp-citation",
  "mcp-financial-data",
  "mcp-gh-source-gate",
  "mcp-ooxml",
  "mcp-pdf",
  "mcp-pptx"
)

# ─── 1. 检查/安装依赖 ──────────────────────────────────────────────
Step 1 "检查/安装系统依赖" {
  if (-not $SkipDeps) {
    # Git
    $haveGit = Get-Command git -ErrorAction SilentlyContinue
    if (-not $haveGit) {
      ColorLine Yellow "  → 安装 Git..."
      winget install --id Git.Git -e --source winget --accept-package-agreements --accept-source-agreements 2>&1 | Out-Null
      $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                  [System.Environment]::GetEnvironmentVariable("Path", "User")
    } else { ColorLine Green "  ✓ Git 已安装" }

    # Rust
    $haveRust = Get-Command rustup -ErrorAction SilentlyContinue
    if (-not $haveRust) {
      ColorLine Yellow "  → 安装 Rust (rustup)..."
      # rustup-init.exe 静默安装
      Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" `
        -OutFile "$env:TEMP\rustup-init.exe"
      & "$env:TEMP\rustup-init.exe" -y --no-modify-path 2>&1 | Out-Null
      Remove-Item "$env:TEMP\rustup-init.exe" -Force -ErrorAction SilentlyContinue
      # 注入 PATH（rustup 会装在 USERPROFILE\.cargo\bin）
      $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    } else { ColorLine Green "  ✓ Rust 已安装" }

    # Node.js（npx 用于 paperplain-mcp）
    $haveNode = Get-Command node -ErrorAction SilentlyContinue
    if (-not $haveNode) {
      ColorLine Yellow "  → 安装 Node.js (LTS)..."
      winget install --id OpenJS.NodeJS.LTS -e --source winget --accept-package-agreements --accept-source-agreements 2>&1 | Out-Null
      $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" +
                  [System.Environment]::GetEnvironmentVariable("Path", "User")
    } else { ColorLine Green "  ✓ Node.js 已安装" }
  } else {
    ColorLine Green "  ✓ 跳过依赖检查 (--SkipDeps)"
  }

  # 最终验证
  $deps = @("git", "rustup", "node")
  foreach ($d in $deps) {
    $ok = Get-Command $d -ErrorAction SilentlyContinue
    if (-not $ok) { throw "缺少依赖: $d" }
  }
  ColorLine Green "  ✓ 所有依赖就绪"
}

# ─── 2. 克隆 / 拉取仓库 ─────────────────────────────────────────
Step 2 "克隆框架仓库" {
  if (Test-Path "$FrameworkRoot\.git") {
    ColorLine Yellow "  → 仓库已存在，执行 git pull"
    Push-Location $FrameworkRoot
    git pull --rebase
    Pop-Location
  } else {
    if (-not $RepoUrl) {
      # 尝试从本地 Mac 远程推断
      ColorLine Yellow "  → 未指定 RepoUrl，尝试从本地远程读取..."
      pushd /Users/joe/Developer/skill 2>$null
      if ($?) {
        $u = git remote get-url origin 2>$null
        if ($u) { $RepoUrl = $u; ColorLine Green "    origin = $RepoUrl" }
        popd
      }
    }
    if (-not $RepoUrl) {
      throw "请提供 -RepoUrl 参数，或确保仓库有 git remote origin"
    }
    # 创建父目录
    $parent = Split-Path $FrameworkRoot -Parent
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }

    ColorLine Yellow "  → 克隆 $RepoUrl"
    git clone --depth 1 $RepoUrl $FrameworkRoot
  }
  ColorLine Green "  ✓ $FrameworkRoot"
}

# ─── 3. 全量编译 Rust 工具 ───────────────────────────────────────
Step 3 "全量编译 Rust 工具（首次约 10-15 分钟）" {
  Push-Location $FrameworkRoot

  ColorLine Yellow "  → cargo build --release (所有 MCP 工具 + router-rs-cli)"
  cargo build --release --features "browser,host-codex,host-cursor,host-claude,host-opencode,research"
  if ($LASTEXITCODE -ne 0) { throw "编译失败" }

  # 验证关键二进制已生成
  $missing = @()
  foreach ($bin in $BINARIES) {
    $p = "target\release\$bin.exe"
    if (-not (Test-Path $p)) { $missing += $bin }
  }
  if ($missing.Count -gt 0) { throw "缺少二进制: $($missing -join ', ')" }
  ColorLine Green "  ✓ 全部 $($BINARIES.Count) 个二进制编译成功"

  Pop-Location
}

# ─── 4. 安装二进制到全局 PATH ──────────────────────────────────
Step 4 "安装二进制到 ~\.cargo\bin" {
  $cargoBin = "$env:USERPROFILE\.cargo\bin"
  if (-not (Test-Path $cargoBin)) { New-Item -ItemType Directory -Path $cargoBin -Force | Out-Null }

  Push-Location $FrameworkRoot
  foreach ($bin in $BINARIES) {
    $src = "target\release\$bin.exe"
    $dst = "$cargoBin\$bin.exe"
    Copy-Item $src $dst -Force
    ColorLine Green "  ✓ $bin.exe"
  }
  Pop-Location

  # 加到当前会话 PATH
  $env:Path = "$cargoBin;$env:Path"

  # 持久化到用户 PATH（避免重启后找不到）
  $curPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if ($curPath -notlike "*$cargoBin*") {
    [Environment]::SetEnvironmentVariable("Path", "$curPath;$cargoBin", "User")
    ColorLine Yellow "  → 已将 $cargoBin 添加到用户 PATH"
  }

  # 验证
  $ver = & router-rs-cli --version 2>&1
  if ($LASTEXITCODE -ne 0) { throw "router-rs-cli 无法运行" }
  ColorLine Green "  ✓ router-rs-cli $ver"
}

# ─── 5. 安装 Codex 宿主投影 ────────────────────────────────────
Step 5 "安装 Codex 宿主投影" {
  Push-Location $FrameworkRoot

  # 确保 .codex 目录存在
  $codexDir = "$env:USERPROFILE\.codex"
  if (-not (Test-Path $codexDir)) { New-Item -ItemType Directory -Path $codexDir -Force | Out-Null }

  ColorLine Yellow "  → framework host-integration install --to codex --scope user"
  # 用刚编译的 release binary 而非 cargo run（更快）
  & "router-rs-cli" framework host-integration install --to codex --scope user

  Pop-Location
  ColorLine Green "  ✓ Codex 投影已安装到 $codexDir"
}

# ─── 6. 全量自检 ──────────────────────────────────────────────
Step 6 "全量自检" {
  Push-Location $FrameworkRoot

  ColorLine Yellow "  → framework doctor"
  & "router-rs-cli" framework doctor --repo-root "$FrameworkRoot"
  if ($LASTEXITCODE -ne 0) { ColorLine Yellow "  ⚠ doctor 发现警告（非致命）" }

  Pop-Location
  ColorLine Green "  ✓ 自检完成"
}

# ─── 7. 彻底清理编译中间产物 ──────────────────────────────────
Step 7 "彻底清理编译中间产物" {
  Push-Location $FrameworkRoot

  # 7a. cargo clean — 删除 target/ 全部构建产物（最大的）
  ColorLine Yellow "  → cargo clean（删除 target/ 构建产物）"
  cargo clean
  ColorLine Green "  ✓ target/ 已删除"

  # 7b. 删除 registry 缓存（.crate 文件和已解压的源码）
  #   - ~\.cargo\registry\cache\ — crates.io 的 .crate 压缩包
  #   - ~\.cargo\registry\src\   — 解压后的源码
  #   index 保留（不费多少空间，重建代价高）
  if (-not $KeepRegistryCache) {
    ColorLine Yellow "  → 清理 cargo registry cache/src（节省 ~500 MB–2 GB）"
    $regCache = "$env:USERPROFILE\.cargo\registry"
    if (Test-Path "$regCache\cache") {
      Remove-Item "$regCache\cache\*" -Recurse -Force -ErrorAction SilentlyContinue
      ColorLine Green "  ✓ ~\.cargo\registry\cache 已清空"
    }
    if (Test-Path "$regCache\src") {
      Remove-Item "$regCache\src\*" -Recurse -Force -ErrorAction SilentlyContinue
      ColorLine Green "  ✓ ~\.cargo\registry\src 已清空"
    }
  } else {
    ColorLine Yellow "  → 保留 registry cache (--KeepRegistryCache)"
  }

  # 7c. 清理 cargo git 临时签出
  $gitDir = "$env:USERPROFILE\.cargo\git"
  if (Test-Path $gitDir) {
    ColorLine Yellow "  → 清理 cargo git checkout"
    Remove-Item "$gitDir\db\*" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item "$gitDir\checkouts\*" -Recurse -Force -ErrorAction SilentlyContinue
    ColorLine Green "  ✓ cargo git 缓存已清空"
  }

  # 7d. npm 缓存（如果在构建中用到了 npm/npx）
  $haveNpm = Get-Command npm -ErrorAction SilentlyContinue
  if ($haveNpm) {
    ColorLine Yellow "  → npm cache clean"
    npm cache clean --force 2>&1 | Out-Null
    ColorLine Green "  ✓ npm 缓存已清除"
  }

  Pop-Location

  # 7e. 框架仓库内非必要的生成文件
  $junkPaths = @(
    "$FrameworkRoot\.supervisor_state.json"
  )
  foreach ($j in $junkPaths) {
    if (Test-Path $j) {
      Remove-Item $j -Force -ErrorAction SilentlyContinue
      ColorLine Green "  ✓ $((Split-Path $j -Leaf)) 已删除"
    }
  }

  # 7f. 清理 Rust/Cargo 的 %TEMP% 残留
  $tempPatterns = @("rust-*", "cargo-*", "rustup-*", "*.rlib")
  $tempDirs = @("$env:TEMP", "$env:TMP")
  foreach ($td in $tempDirs) {
    if (-not (Test-Path $td)) { continue }
    foreach ($pat in $tempPatterns) {
      Get-ChildItem "$td\$pat" -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        Remove-Item $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
        ColorLine Green "  ✓ $($_.Name) (TEMP)"
      }
    }
  }

  # 7g. 清理空的残留目录
  $empties = @(
    "$env:USERPROFILE\.cargo\registry\cache",
    "$env:USERPROFILE\.cargo\registry\src",
    "$env:USERPROFILE\.cargo\git\db",
    "$env:USERPROFILE\.cargo\git\checkouts"
  )
  foreach ($e in $empties) {
    if (Test-Path $e) {
      $children = Get-ChildItem $e -ErrorAction SilentlyContinue
      if (-not $children) { Remove-Item $e -Force -ErrorAction SilentlyContinue }
    }
  }
}

# ─── 完成 ────────────────────────────────────────────────────────
ColorLine Green "`n╔══════════════════════════════════════════════════════════╗"
ColorLine Green "║          Skill Harness 安装完成！                      ║"
ColorLine Green "╚══════════════════════════════════════════════════════════╝"

$disk = Get-PSDrive $FrameworkRoot[0]
$free = [math]::Round($disk.Free / 1GB, 1)
ColorLine Cyan "    磁盘剩余: ${free} GB"
ColorLine Cyan "    框架路径: $FrameworkRoot"
ColorLine Cyan "    二进制位置: $env:USERPROFILE\.cargo\bin\"
ColorLine Cyan "    Codex 配置: $env:USERPROFILE\.codex\"

ColorLine Yellow "`n下一步操作："
ColorLine Yellow "  1. 打开一个新的 PowerShell（刷新 PATH）"
ColorLine Yellow "  2. cd $FrameworkRoot"
ColorLine Yellow "  3. codex '使用 \$research 做个文献调研'"
ColorLine Yellow "  或直接在工作目录中启动 Codex："
ColorLine Yellow "     codex"
ColorLine Yellow ""
ColorLine Yellow "  快速验证工具是否就绪："
ColorLine Yellow "     router-rs-cli framework doctor --repo-root '$FrameworkRoot'"
