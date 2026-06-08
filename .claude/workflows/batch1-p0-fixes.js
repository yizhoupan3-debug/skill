import { agent, parallel, pipeline, phase, log } from "workflow"
export const meta = {
  name: 'batch1-p0-security-fixes',
  description: 'Batch 1: P0安全修复 — 路径遍历防护、fsync对齐、死函数删除',
  phases: [
    { title: 'P0修复', detail: '4项安全/完整性修复' },
    { title: '验证', detail: 'cargo check + 测试' },
  ],
}

// Batch 1: 4 P0 fixes, each as an independent agent
// Using pipeline to process sequentially (3 agents per item: fix, verify, commit-msg)

const fixes = [
  {
    name: 'path_traversal_guard',
    prompt: `面向用户的可见输出使用简体中文。

修复 core/router-rs/src/cli/runtime_ops.inc 中的路径遍历漏洞。

## 问题
write_transport_binding_payload（约第2151行）和 write_checkpoint_resume_manifest_payload（约第2165行）直接使用 JSON payload 中的 path 参数写入文件，无路径边界校验。

## 修复方案
参考同仓库 runtime_storage.rs 中的 filesystem_reject_symlink_write_target 模式。

在 write_text_payload 函数（约第2196行）中，写入前添加路径校验：
1. canonicalize 父目录
2. 确认 canonicalized 路径在 repo root 下（strip_prefix 校验）
3. 拒绝包含 .. 路径段的路径
4. 检查符号链接

具体实现：在 write_text_payload 函数中，在 fs::create_dir_all(parent) 之前添加校验逻辑。参考 runtime_storage.rs 第805行附近的模式。

注意：这个文件是 .inc 文件（被 include! 到其他文件中），确保修改后语法正确。

## 完成后
输出修改的文件路径和行号，以及修改前后的代码对比。`,
  },
  {
    name: 'fsync_background_state',
    prompt: `面向用户的可见输出使用简体中文。

修复 core/router-rs/src/background_state.rs 中 write_persisted_state 函数的 fsync 缺失。

## 问题
约第1394-1425行，filesystem 分支使用 fs::write + fs::rename 模式但缺少 sync_all()。

## 修复方案
将 fs::write(&tmp_path, payload) 改为：
1. OpenOptions::new().create(true).write(true).truncate(true).open(&tmp_path)
2. file.write_all(payload.as_bytes())
3. file.sync_all()
4. fs::rename(&tmp_path, state_path)

参考同文件中可能已有的模式，或 session_supervisor.rs 第919行附近的写法。

## 完成后
输出修改内容和行号。`,
  },
  {
    name: 'fsync_task_ledger',
    prompt: `面向用户的可见输出使用简体中文。

修复 core/antigravity/src/task_ledger.rs 中 append_transaction_assuming_l1_held 的 fsync 缺失。

## 问题
约第94-167行，两个分支（文件已存在/文件不存在）都在 writeln! 后直接 drop file，无 sync。

## 修复方案
在 writeln!(file, "{}", serialized)?; 之后、函数返回之前，添加：
file.sync_all().map_err(|e| format!("fsync task_ledger failed: {e}"))?;

注意：
- file 变量需要是 mut 才能调用 sync_all（已经是 mut）
- 这是低优先级修复（append-only 格式），但仍应与其他写入模式对齐

## 完成后
输出修改内容和行号。`,
  },
  {
    name: 'dead_code_removal',
    prompt: `面向用户的可见输出使用简体中文。

删除 core/router-rs/src/framework_runtime/session_artifacts.rs 中的两个死函数。

## 问题
- write_json_artifact_if_changed（约第484行）：#[allow(dead_code)]，全仓库零调用
- sha256_json（约第740行）：#[allow(dead_code)]，全仓库零调用

## 修复方案
1. 删除 write_json_artifact_if_changed 函数定义（约第484-510行）及其 #[allow(dead_code)] 标注
2. 删除 sha256_json 函数定义（约第740-755行）及其 #[allow(dead_code)] 标注
3. 如果这两个函数对应的测试也存在，一并删除

## 注意
- 先读取文件确认确切的行号范围
- 删除后确认没有编译错误（检查是否有 test 模块引用了这些函数）
- 如果测试模块中有对这两个函数的测试，也需要删除

## 完成后
输出删除的内容摘要和行号。`,
  },
]

// Run fixes sequentially, each fix → verify
for (const fix of fixes) {
  log(`▸ 修复: ${fix.name}`)
  await agent(fix.prompt, {
    label: `fix:${fix.name}`,
    phase: 'P0修复',
  })
  log(`✓ 完成: ${fix.name}`)
}

// Verify all fixes compile
log('=== 验证编译 ===')
const buildResult = await agent(`面向用户的可见输出使用简体中文。

运行 cargo check 验证所有 P0 修复是否编译通过：
1. cd /Users/joe/Developer/skill/core/router-rs && cargo check 2>&1
2. cd /Users/joe/Developer/skill/core/antigravity && cargo check 2>&1

如果编译失败，分析错误并报告需要的额外修复。

输出 cargo check 的结果摘要。`, {
  label: 'verify:cargo-check',
  phase: '验证',
})

log('=== Batch 1 完成 ===')
return { fixes: fixes.map(f => f.name), buildResult }
