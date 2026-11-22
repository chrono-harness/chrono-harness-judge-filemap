# chrono-harness-judge-filemap

根据登记图计算变更影响和必须运行的测试。

运行 `./check.sh` 验证独立组件。证明使用 Verus 0.2026.04.24.f8e1704，通过环境变量 `VERUS` 指定可执行文件。

[固定来源](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/judge-filemap)

`chrono-judge-filemap impact-summary` 从标准输入读取一个完整的 FILEMAP impact JSON。它先用本仓的 `Impact` 类型解析整个输入，再向标准输出写一个 JSON 对象。项目判官读取按键排序的 `reached` 和原序的 `retired_tests`。成本判官还需要原始 `schema`、`delta`、`tests` 和按原序投影为节点名的 `required_tests`。该命令保留输入的 schema 字符串，由使用方决定支持的版本。输入或输出失败时错误写入 stderr，退出码为 2；成功时为 0。

`chrono-judge-filemap endpoint-tests` 在宿主仓根目录运行。标准输入为含 `config`、`base`、`candidate` 的 JSON 对象，分别提供登记入口和两个完整 Git 提交 ID。命令读取固定端点的登记、文件树和显式语义文档，返回 `tests`、`retired_tests` 与 `findings`。调用方须处理全部 findings，发现 error 时停止采用选择结果。读取或解码失败时返回 2 并写入标准错误。
