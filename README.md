# chrono-harness-judge-filemap

根据登记图计算变更影响和必须运行的测试。

运行 `./check.sh` 验证独立组件。证明使用 Verus 0.2026.04.24.f8e1704，通过环境变量 `VERUS` 指定可执行文件。

[固定来源](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/judge-filemap)

`chrono-judge-filemap impact-summary` 从标准输入读取一个完整的 FILEMAP impact JSON。它先用本仓的 `Impact` 类型解析整个输入，再向标准输出写一个 JSON 对象。项目判官读取按键排序的 `reached` 和原序的 `retired_tests`。成本判官还需要原始 `schema`、`delta`、`tests` 和按原序投影为节点名的 `required_tests`。该命令保留输入的 schema 字符串，由使用方决定支持的版本。输入或输出失败时错误写入 stderr，退出码为 2；成功时为 0。

`chrono-judge-filemap endpoint-tests` 在宿主仓根目录运行。标准输入为含 `config`、`base`、`candidate` 的 JSON 对象，分别提供登记入口和两个完整 Git 提交 ID。命令读取固定端点的登记、文件树和显式语义文档，返回 `tests`、`retired_tests` 与 `findings`。调用方须处理全部 findings，发现 error 时停止采用选择结果。读取或解码失败时返回 2 并写入标准错误。

`endpoint-tests` 和判官入口都通过宿主 PATH 中固定安装的 `chrono-judge-mixed` 查询语义文档和规则分类。FILEMAP 在本次 Git 读取上下文取得这些指定文件，使用返回的规则路径选择工作流测试。只运行 `impact-summary` 时可单独安装 FILEMAP。

`closure-v1` 从 stdin 接收两个端点的边数组 `base`、`candidate`，起点数组 `seeds` 与布尔值 `reachable_only`。每条边有 `from`、`kind`、`to`，每个起点有 `id`、`node`、`reference`、`reason`。同一 ID 可以重复指向同一节点。一个 ID 指向两个不同节点时，两种查询模式都以状态 2 退出，在 stderr 报告 `E_SEED_ID_CONFLICT` 和冲突的两个节点。stdout 返回 `selected_tests`、`reached` 和带端点来源的 `edges`。选 true 时只返回从可达节点出发的边。选 false 时返回全部边及含前驱和遍历记录的 `closure`。这个查询可单独离线运行。

完整 CI 调度调用 `endpoint-tests` 时可传 `"scheduling":true`。FILEMAP 按调度规则解释两个端点，并额外返回 `reached`、`changed_paths` 与自己的 `git_facts`。`collection-tests-v1` 在宿主根目录接收 `request` 和可选的 `migration`。前者是绑定当前检出与 collect 范围的请求，后者包含登记要求的 `units` 和原始 `originals` 清单。收集时验证有效输入，返回 `tests`、`retired_tests`、`findings`、`reached` 与 `git_facts`。调用方须处理 error 级 findings。两种查询均从 stdin 读取 JSON，成功退出 0，失败退出 2 并向 stderr 写原因。

FILEMAP 的端点查询、调度和收集从 PATH 调用固定发行的 `chrono-judge-registration impact-views-v1`。FILEMAP 在两个提交上取得登记原文，登记工具按公开协议返回节点定义、测试归属和解释后的登记。FILEMAP 据此计算闭包和测试选择。只查询 `impact-summary` 或 `closure-v1` 时可单独使用 FILEMAP。

测试从 `tests/fixtures/registration-standard.json` 读取公开响应的端点样例，`registration-variants.json` 按命名场景用 JSON Patch 保存相对改动。两份文件位于 `crates/judge-filemap-tests/` 下，供影响分析与命令边界测试离线使用。

完整调度重新打开 Git 读取器时，FILEMAP 将已观察的继承环境传给 registration。未设置的变量和配置排除的凭据保持缺席。配置中的字面值由读取器从各端点的配置解析。

`chrono-judge-filemap cost-inputs-v1` 为成本判官准备数据。它接收已封存的判官请求，从 registration 的 `filemap-views` 读取两端按原顺序保存的文件和节点，核对前置 FILEMAP 结果和完整 DELTA，返回受影响节点、测试、绑定与原始 Git 观察。

本仓的产品和测试通过 crates.io 依赖独立构建。原生判官和 `cost-inputs-v1` 从宿主 PATH 选择已安装的 runner，使用其封存请求、限额和原生响应入口。固定提交的 Git 读取在一次 `git-facts-v1` 会话内完成，registration 和 mixed 通过 `run-command-v1` 在宿主指定的时间、输出上限和可执行文件摘要下运行。调用结束时关闭读取会话并收取实际退出状态。独立使用 `impact-summary`、`closure-v1` 和 `--version` 只需 FILEMAP 自身。

`closure-v1` 和 impact 共用有限拓扑上的所有起点遍历。每个到达节点和原有类型边的原因集合恰好是能够到达它的输入起点 ID 集合。相同 ID 去重，互不相连的起点只作用于各自可达的部分。完整循环的验证绑定实际 FIFO 下标、原邻接行和公共报告使用的稀疏表。前驱树在每个起点完成时交给报告层。

impact 从实际遍历原边一次生成 `required_tests`、`tests` 和 `retired_tests`。`test-execution` 目标按原节点名排序并去重，每个目标附上全部匹配原边。候选登记中确有测试节点的目标进入 `tests`，其余进入 `retired_tests`。

变更文件、登记目标及有变化的边生成原因时，起点身份使用 JSON 的 `[reference, reason, node]`。Verus 核对实际传入编码器的字段顺序、所有 DELTA 文件及有变化边的两个端点。外部 JSON 字符串编码由 serde_json 提供，原生测试解码实际输出并核对含引号、反斜杠、换行及 Unicode 的身份。
