# Five Towers And Hero Skill Expansion

英雄触发规则已由[职业施法规格](2026-09-07-class-auto-casts.md)取代。本文件保留五塔收敛范围与当时的历史验收记录；旧版本测试、构建和截图不能证明新被动版本已通过验收。

## 目标

将可建造防御塔从 19 座收敛为 5 座，并把其余 14 座塔的核心玩法迁移到九种英雄武器中，让基础塔阵清晰、英雄选择明显改变战斗方式。

## 范围内

- 保留箭塔、炮塔、魔法塔、冰塔、侦测塔。
- 删除其余 14 个塔枚举、定义、专属战斗分支、建造入口、档案内容和运行时塔素材。
- 建造栏改成单组 5 塔，数字快捷键收敛为 `1`-`5`。
- 九种英雄武器吸收旧塔机制：
  - 战旗长剑：风塔的冲锋、击退和眩晕。
  - 星火法杖：激光塔、火塔和冰霜新星的聚焦、燃烧区域和终段冻结。
  - 猎影长弓：狙击塔和毒塔的远程处决、穿透和持续毒伤。
  - 誓约盾锤：要塞炮和圣光塔的壁垒、修塔、治疗萝卜和防御增益。
  - 雷暴法器：雷塔的多目标连锁雷击。
  - 哨戒弩：光棱塔的折射结界，并保留反隐能力。
  - 夜刃匕首：暗影塔的位移刺杀、死印和破甲诅咒。
  - 召唤法杖：召唤塔和死灵塔的眷属召唤及亡者复苏。
  - 工匠战锤：导弹塔的机械守卫和追踪火箭齐射。
- 清理或改写只对已删除塔有意义的遗物提示和英雄说明。
- 更新模拟器、截图 harness、README 和玩法指南。

## 明确不做

- 不增加第二套英雄职业或新的技能选择页面。
- 不保留旧塔兼容层；关卡内塔布局不会跨局保存。
- 不扩充怪物、关卡或装备数量。
- 不重做现有英雄、怪物或装备视觉资产体系。

## 假设

- 保留阵容采用用户确认的方案 A：箭塔、炮塔、魔法塔、冰塔、侦测塔。
- 九种英雄武器保留；技能加点和英雄存档结构以当前职业施法规格为准。
- 旧塔的能力迁移以玩法辨识度为目标，不逐项复制旧塔数值。
- 英雄武器按当前职业施法规格自动触发，不提供手动技能入口。
- 侦测塔继续作为不依赖英雄选择的稳定反隐方案。

## 行为场景

### Requirement: 五塔基础阵容

- WHEN 玩家进入战斗或防御塔档案，THEN 只看到箭塔、炮塔、魔法塔、冰塔和侦测塔。
- WHEN 玩家按数字键 `1`-`5`，THEN 分别选择上述五种塔；其他数字键不选择塔。
- WHEN 关卡出现飞行或隐形敌人，THEN 五塔阵容仍有对空和反隐手段，不强制选择特定英雄。

### Requirement: 英雄技能继承旧塔玩法

- WHEN 玩家切换九种武器，THEN 每种被动具有对应旧塔机制带来的独特目标、空间或持续效果，而不是只有不同颜色的范围伤害。
- 自动触发、秒数间隔、无目标等待、召唤上限和移动保护以职业施法规格为准。

### Requirement: 删除旧内容

- WHEN 代码、档案、文档和运行时素材被检查，THEN 不再存在 14 座已删除塔的玩家可见内容或不可达专属实现。
- WHEN 玩家查看遗物提示，THEN 推荐对象只引用仍存在的五座塔或通用英雄/塔定位。

## 验收标准

- `TowerKind::ALL` 和塔定义表都严格包含 5 项，且搜索不到 14 个旧 `TowerKind` 变体。
- `cargo fmt --check`、`cargo test --all-targets`、`cargo check` 通过。
- `./build-web.sh` 通过，证明 Web 打包与剩余素材加载有效。
- 模拟器 `iso` 输出恰好 5 座塔；九种英雄构筑 `builds` 使用真实被动系统；全关卡 `all` sweep 输出无超时并形成可检查的难度曲线。
- 截图 harness 证明建造栏只显示 5 塔，并覆盖英雄被动生效后的战场状态；桌面和手机视口中核心战场及控件不互相遮挡。
- 运行时素材校验不再要求 14 座已删除塔的图片。
- README 和玩法指南不再宣称 19 塔或旧快捷键/旧主动技。

## 风险

- 塔枚举被渲染、模拟器、装备和特效广泛引用，直接删除会暴露大量穷尽匹配；必须逐个判断是删除、泛化还是迁入英雄技能。
- 五塔经济和输出结构与原 19 塔差异大，需要用复用真实战斗系统的模拟器调平，不能仅凭静态数值判断。
- 英雄武器吸收多种旧塔能力后需要同时检查目标命中、触发间隔、召唤数量和关卡曲线。

## 历史验收证据

以下记录对应五塔收敛完成时的版本，不是当前职业施法变更的验收结果。新结果记录在职业施法规格中。

- PASS 代码与内容清理：`TowerKind::ALL`、塔定义、建造栏和快捷键均只保留箭塔、炮塔、魔法塔、冰塔、侦测塔；针对 14 个已删除 `TowerKind` 变体、`6`-`9` 建塔快捷键、19 塔文案和旧塔素材路径的残留搜索结果为空。`assets/sprites/towers/` 中运行时 WebP 恰好为上述 5 个文件。
- PASS Rust 验证：Nix 开发环境内执行 `CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo test --all-targets`，59 项通过（库测试 45、模拟器测试 3、技能集成测试 11），0 失败；日志为 `tmp/five-towers-tests-limited.log`。`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 cargo check --all-targets`、`cargo fmt --all -- --check`、`git diff --check`、`bash -n build-web.sh` 均通过。仅有 vendored `bevy_firefly` 的 5 条既有 warning。
- 当时的九种效果测试验证了目标、空间和持续效果，包括火箭飞行、死亡目标重新追踪、溅射及燃烧，以及已阵亡非首领亡魂的一次性消耗。当前回归测试已迁至 `tests/hero_passives.rs`，旧结果不覆盖自动触发条件。
- PASS 当时的 Web 打包：`CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 ./build-web.sh` 已完成，后台任务 `carrot-five-towers-web-final-0906` 为 `inactive / success / ExecMainStatus=0`，日志为 `tmp/five-towers-web-final.log`。脚本只构建游戏主程序，不再把原生 capture/sim 工具编入 WASM。当时 WASM 为 25908856 字节，gzip 约 9.4 MB、Brotli 约 6.2 MB；`web/assets/sprites/towers/` 恰好有五张 WebP。WASM SHA-256 为 `4ef4c1801fe01cd1ab0ac4967ce3db747e67dd89d0acd588bfcb1fb89fd056ca`。此项只证明历史构建与打包，不等同于新被动构建或浏览器渲染通过。
- PASS 塔隔离模拟：`./target/debug/sim 0 24680 iso` 只输出 5 行塔数据；箭塔、炮塔、魔法塔、冰塔通关，纯侦测塔按其辅助定位在第 2/5 波失败。日志为 `tmp/five-towers-iso.log`。
- PASS 英雄构筑模拟：`./target/debug/sim 0 24680 builds` 完成基线和九种武器构筑；九种构筑均实际释放技能（每局平均 3-5 次），并在该早期关卡的 8 个种子中全部通关。日志为 `tmp/five-towers-builds.log`，后台任务退出码为 0。
- PASS 当时的全关卡模拟：`./target/debug/sim 0 24680 all 2` 跑完100关、每关2个种子，共200局，后台任务 `carrot-five-towers-sweep-0906` 退出码为0。关卡编号严格为1-100，无缺行或重复，超时合计为0。逐关胜率、平均波数、平均生命和超时数记录于 `tmp/five-towers-sweep.log`。模拟使用一级、无装备基线英雄；39关胜率100%、7关胜率50%、54关胜率0%。五章平均胜率依次为50%、40%、45%、40%、37.5%，不代表成长构筑或新被动版本的最终胜率，不能据此宣称全曲线已平衡。
- PASS 当时的桌面和移动端截图 harness：两套视口均完成九种武器效果场景；同时确认4个神话眷属、5个固定守卫，英雄移动后守卫仍锚定原位。历史截图为 `screenshots/five-towers-skills/verified-desktop.png`（1280×720，170551种颜色）和 `screenshots/five-towers-skills/verified-mobile.png`（844×390，83268种颜色）。五塔保持单行；桌面顶部按钮不遮挡金币、生命和波次，手机左右侧栏保留核心战场空间。日志为 `tmp/five-towers-desktop.log`、`tmp/five-towers-mobile.log`。这些截图不证明当前被动HUD或自动触发已验收。
- PASS 浏览器启动与导航：通过 Chrome 控制技能，在实际 Chrome 的独立验收标签中加载最终 Web 包，确认进入按钮、关卡菜单、剧情推进、剧情路线选择、战前部署入口均能操作。菜单截图为 `screenshots/five-towers-skills/chrome-final-desktop.png`（1280×720，56611 种颜色），剧情截图为 `screenshots/five-towers-skills/chrome-final-story.png`（2178×1371，450453 种颜色）。视口覆盖曾使画布缓冲区与输入缩放不一致；恢复 Chrome 原生缩放并刷新后，第一关点击正常进入剧情，无需修改游戏代码。
- 未验证 浏览器战斗视觉：进入战斗后，实际 Chrome 的桌面及 844×390 横屏触控截图请求均超过 20 秒超时，尚未取得有效战斗帧，不能宣称 Web 战斗完整验收通过。当前战斗视觉证据仅为上述原生 harness。独立 Chromium/Chrome 自动化后端另出现过 SwiftShader 单色黑屏、首次适配器初始化失败，以及 Vulkan `VK_ERROR_OUT_OF_DEVICE_MEMORY` / `DeviceLost`；旧 Web 包预检查也未取得有效画面，尚不能把这些问题归因于本次五塔改动。`browser-final-desktop-menu-retry.png`、`browser-final-mobile-unverified.png` 的 `colors=1`，不得作为通过证据；`chrome-final-battle.png` 实际仍为菜单，也不得当作战斗截图。所有本轮创建的测试浏览器和验收标签已关闭，临时视口设置已清理，用户原有标签保留。
- PASS 内容与素材审计：`python3 tools/audit_release_content.py` 为 35/35 PASS，证据为 `tmp/content_audit.json`、`tmp/content_audit.md`。默认校验实际发布的 WebP 素材，141/141 通过：塔 5、敌人原型图片 16、装备 20、物种肖像 100。18 种敌人行为原型复用 16 张图片，第 101 个物种（ID 100）复用肖像 099；StoryOS/Comfy manifest 同样为 141 条，不重复生成共享图片。
- PASS 压缩包与 HTTP 素材交付：Node `zlib` 解压 gzip 和 Brotli 后，逐字节比对最终 WASM 均一致；经 `http://127.0.0.1:8765/` 获取五张塔图均返回 HTTP 200，响应内容与 `assets/sprites/towers/` 对应文件逐字节一致。

### 基线难度曲线摘要

| 关卡 | 平均胜率 | 0% 胜率关数 | 超时 | 章节终关平均存活波数 |
| --- | ---: | ---: | ---: | ---: |
| 1-20 | 50% | 9 | 0 | 7.5/20 |
| 21-40 | 40% | 11 | 0 | 4.0/20 |
| 41-60 | 45% | 11 | 0 | 4.0/20 |
| 61-80 | 40% | 11 | 0 | 5.0/20 |
| 81-100 | 37.5% | 12 | 0 | 2.5/20 |
