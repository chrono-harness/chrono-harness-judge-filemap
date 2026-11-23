# chrono-harness-judge-filemap

根据登记图计算变更影响和必须运行的测试。

运行 `./check.sh` 验证独立组件。证明使用 Verus 0.2026.04.24.f8e1704，通过环境变量 `VERUS` 指定可执行文件。

[固定来源](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/judge-filemap)

`chrono-judge-filemap impact-summary` 从标准输入读取一个完整的 FILEMAP impact JSON。它先用本仓的 `Impact` 类型解析整个输入，再向标准输出写一个 JSON 对象。项目判官读取按键排序的 `reached` 和原序的 `retired_tests`。成本判官还需要原始 `schema`、`delta`、`tests` 和按原序投影为节点名的 `required_tests`。该命令保留输入的 schema 字符串，由使用方决定支持的版本。输入或输出失败时错误写入 stderr，退出码为 2；成功时为 0。

`chrono-judge-filemap endpoint-tests` 在宿主仓根目录运行。标准输入为含 `config`、`base`、`candidate` 的 JSON 对象，分别提供登记入口和两个完整 Git 提交 ID。命令读取固定端点的登记、文件树和显式语义文档，返回 `tests`、`retired_tests` 与 `findings`。调用方须处理全部 findings，发现 error 时停止采用选择结果。读取或解码失败时返回 2 并写入标准错误。

`endpoint-tests` 和判官入口都通过宿主 PATH 中固定安装的 `chrono-judge-mixed` 查询语义文档和规则分类。FILEMAP 在本次 Git 读取上下文取得这些指定文件，使用返回的规则路径选择工作流测试。只运行 `impact-summary` 时可单独安装 FILEMAP。

`closure-v1` 从 stdin 接收两个端点的边数组 `base`、`candidate`，起点数组 `seeds` 与布尔值 `reachable_only`。每条边有 `from`、`kind`、`to`，每个起点有 `id`、`node`、`reference`、`reason`。stdout 返回 `selected_tests`、`reached` 和带端点来源的 `edges`。选 true 时只返回从可达节点出发的边。选 false 时返回全部边及含前驱和遍历记录的 `closure`。这个查询可单独离线运行。

完整 CI 调度调用 `endpoint-tests` 时可传 `"scheduling":true`。FILEMAP 按调度规则解释两个端点，并额外返回 `reached`、`changed_paths` 与自己的 `git_facts`。`collection-tests-v1` 在宿主根目录接收 `request` 和可选的 `migration`。前者是绑定当前检出与 collect 范围的请求，后者包含登记要求的 `units` 和原始 `originals` 清单。收集时验证有效输入，返回 `tests`、`retired_tests`、`findings`、`reached` 与 `git_facts`。调用方须处理 error 级 findings。两种查询均从 stdin 读取 JSON，成功退出 0，失败退出 2 并向 stderr 写原因。

FILEMAP 的端点查询、调度和收集从 PATH 调用固定发行的 `chrono-judge-registration impact-views-v1`。FILEMAP 在两个提交上取得登记原文，登记工具按公开协议返回节点定义、测试归属和解释后的登记。FILEMAP 据此计算闭包和测试选择。只查询 `impact-summary` 或 `closure-v1` 时可单独使用 FILEMAP。

测试从 `tests/fixtures/registration-standard.json` 读取公开响应的端点样例，`registration-variants.json` 按命名场景用 JSON Patch 保存相对改动。两份文件位于 `crates/judge-filemap-tests/` 下，供影响分析与命令边界测试离线使用。
