# chrono-harness-judge-filemap

根据登记图计算变更影响和必须运行的测试。

在仓库根目录运行 `mise exec -- ./check.sh`。检查从本仓的 mise 配置安装固定版本的 Verus，按变更选择图与键比较的证明及其原生消费者。

[固定来源](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/judge-filemap)

`chrono-judge-filemap impact-summary` 从标准输入读取一个完整的 FILEMAP impact JSON。它先用本仓的 `Impact` 类型解析整个输入，再向标准输出写一个 JSON 对象。项目判官读取按键排序的 `reached` 和原序的 `retired_tests`。成本判官还需要原始 `schema`、`delta`、`tests` 和按原序投影为节点名的 `required_tests`。该命令保留输入的 schema 字符串，由使用方决定支持的版本。输入或输出失败时错误写入 stderr，退出码为 2；成功时为 0。

`chrono-judge-filemap endpoint-tests` 在宿主仓根目录运行。标准输入为含 `config`、`base`、`candidate` 的 JSON 对象，分别提供登记入口和两个完整 Git 提交 ID。命令读取固定端点的登记、文件树和显式语义文档，返回 `tests`、`retired_tests` 与 `findings`。调用方须处理全部 findings，发现 error 时停止采用选择结果。读取或解码失败时返回 2 并写入标准错误。

`endpoint-tests` 和判官入口都通过宿主 PATH 中固定安装的 `chrono-judge-mixed` 查询语义文档和规则分类。FILEMAP 在本次 Git 读取上下文取得这些指定文件，使用返回的规则路径选择工作流测试。只运行 `impact-summary` 时可单独安装 FILEMAP。

`closure-v1` 从 stdin 接收两个端点的边数组 `base`、`candidate`，起点数组 `seeds` 与布尔值 `reachable_only`。每条边有 `from`、`kind`、`to`，每个起点有 `id`、`node`、`reference`、`reason`。同一 ID 可以重复指向同一节点。一个 ID 指向两个不同节点时，两种查询模式都以状态 2 退出，在 stderr 报告 `E_SEED_ID_CONFLICT` 和冲突的两个节点。stdout 返回 `selected_tests`、`reached` 和带端点来源的 `edges`。选 true 时只返回从可达节点出发的边。选 false 时返回全部边及含前驱和遍历记录的 `closure`。这个查询可单独离线运行。

完整 CI 调度调用 `endpoint-tests` 时可传 `"scheduling":true`。FILEMAP 按调度规则解释两个端点，并额外返回 `reached`、`changed_paths` 与自己的 `git_facts`。`collection-tests-v1` 在宿主根目录接收 `request` 和可选的 `migration`。前者是绑定当前检出与 collect 范围的请求，后者包含登记要求的 `units` 和原始 `originals` 清单。收集时验证有效输入，返回 `tests`、`retired_tests`、`findings`、`reached` 与 `git_facts`。调用方须处理 error 级 findings。两种查询均从 stdin 读取 JSON，成功退出 0，失败退出 2 并向 stderr 写原因。

正式评估与离线收集都先核对基线树、候选树和完整的有序 DELTA。登记回复必须保留两个原提交及适用的请求 ID。正式评估还需要登记视图。在评估、Git 观察或关闭同时失败时，正式判官按观察、关闭、评估的顺序返回首项错误，并保留已经取得的 Git 记录。错误首字段以 `E_SEMANTIC_` 开头时沿用该错误码，其余评估错误使用 `E_INPUT`。

FILEMAP 的端点查询、调度和收集从 PATH 调用固定发行的 `chrono-judge-registration impact-views-v1`。FILEMAP 在两个提交上取得登记原文，登记工具按公开协议返回节点定义、测试归属和解释后的登记。FILEMAP 据此计算闭包和测试选择。只查询 `impact-summary` 或 `closure-v1` 时可单独使用 FILEMAP。

测试从 `tests/fixtures/registration-standard.json` 读取公开响应的端点样例，`registration-variants.json` 按命名场景用 JSON Patch 保存相对改动。两份文件位于 `crates/judge-filemap-tests/` 下，供影响分析与命令边界测试离线使用。

完整调度重新打开 Git 读取器时，FILEMAP 将已观察的继承环境传给 registration。未设置的变量和配置排除的凭据保持缺席。配置中的字面值由读取器从各端点的配置解析。

`chrono-judge-filemap cost-inputs-v1` 为成本判官准备数据。它接收已封存的判官请求，从 registration 的 `filemap-views` 读取两端按原顺序保存的文件和节点，核对前置 FILEMAP 结果和完整 DELTA。它至少需要一份匹配的前置影响，其他返回影响的前置项也要逐一相同；任一层的重复登记回复字段都会导致错误。JSON 字段读取和完整值相等由 serde 适配器提供，两份 Verus 根验证其上的前置结果选择。成功后返回受影响节点、测试、绑定与原始 Git 观察。

本仓的产品和测试通过 crates.io 依赖独立构建。原生判官和 `cost-inputs-v1` 从宿主 PATH 选择已安装的 runner，使用其封存请求、限额和原生响应入口。固定提交的 Git 读取在一次 `git-facts-v1` 会话内完成，registration 和 mixed 通过 `run-command-v1` 在宿主指定的时间、输出上限和可执行文件摘要下运行。调用结束时关闭读取会话并收取实际退出状态。独立使用 `impact-summary`、`closure-v1` 和 `--version` 只需 FILEMAP 自身。

`closure-v1` 和 impact 共用有限拓扑上的所有起点遍历。每个到达节点和原有类型边的原因集合恰好是能够到达它的输入起点 ID 集合。相同 ID 去重，互不相连的起点只作用于各自可达的部分。完整循环的验证绑定实际 FIFO 下标、原邻接行和公共报告使用的稀疏表。每个通过起点校验的 ID 在最终公开的 `closure.predecessors` 字典中都有条目，条目恰好保留对应 FIFO 的根和全部前驱边。重复同 ID、同根的记录保留同一棵树。Verus 验证实际字典的每次写入及最终返回值，标准库和 serde_json 负责将该值编码为 JSON。

impact 从实际遍历原边一次生成 `required_tests`、`tests` 和 `retired_tests`。`test-execution` 目标按原节点名排序并去重，每个目标附上全部匹配原边。候选登记中确有测试节点的目标进入 `tests`，其余进入 `retired_tests`。

变更文件、登记目标及有变化的边生成原因时，起点身份使用 JSON 的 `[reference, reason, node]`。Verus 核对实际传入编码器的字段顺序、所有 DELTA 文件及有变化边的两个端点。外部 JSON 字符串编码由 serde_json 提供，原生测试解码实际输出并核对含引号、反斜杠、换行及 Unicode 的身份。

## 本仓本地检查的清理

开发者运行 `mise exec -- ./check.sh` 时，工作树租约覆盖整个检查。准备输入和原报告写入 `.chrono-harness/state/preparation/`，单元结果在 `.chrono-harness/state/filemap/`，汇总在 `.chrono-harness/state/collection/`。登记的准备动作先创建 `.chrono-harness/state/filemap-proofs/`。两组证明编译库及依赖清单写入此目录，原生检查随后按文件核验编译器输入。维护轮次在检查结束后回收此目录。原生构建使用专属的 `native-check/` 临时目录。需要保留的结果应在当日清理前复制到持久存储。

在主检出运行 `mise exec -- chrono-worktree maintain --host-root . --config .chrono-harness/worktree.json
```` 可执行一轮工作树维护。独占执行者核对身份、租约与源码保全情况，然后回收符合条件的产物。

同一日历批次共用截止时刻。调度器在运行且取得排他租约时回收已登记的空闲目录。首次登记前已有的目录需要核对、转存并显式接管。普通任务工作树在超过 24 小时且唯一源码已保全后可整树回收；仍在使用的原件继续保管。工作树日志和处置报告在一天的窗口及当前使用方结束后滚动。工具包位于 mise 共享安装目录，本仓的 bin/ 只保存链接。
