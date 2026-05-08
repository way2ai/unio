# Unio

<section class="unio-hero">
  <div class="unio-kicker">本地智能体运行时</div>
  <h1>在一个终端智能体中构建、检查与自动化。</h1>
  <p>
    Unio 是一个 Rust workspace，用于构建通用智能体。它把 CLI、本地
    daemon、模型提供方、工具执行、审批策略、技能、存储与追踪事件组织成
    一个完整的开发者工作流。
  </p>
  <div class="unio-actions">
    <a href="install.html">安装</a>
    <a href="quickstart.html">快速开始</a>
    <a href="architecture.html">架构</a>
  </div>
  <pre><code>cargo run -p unio -- exec "inspect README.md"
cargo run -p unio -- tool read --args path=README.md</code></pre>
</section>

<section class="unio-grid">
  <article>
    <h2>智能体工作流</h2>
    <p>运行提示词、恢复会话、查看 trace，并把对话状态绑定到当前工作区。</p>
  </article>
  <article>
    <h2>带策略的工具</h2>
    <p>通过审批层读取文件、写入文件和执行已注册工具，而不是直接暴露不受控的 shell。</p>
  </article>
  <article>
    <h2>技能</h2>
    <p>从 `.unio/skills/` 加载仓库或用户技能，并通过统一的工具合同调用。</p>
  </article>
  <article>
    <h2>可观测运行</h2>
    <p>trace ID、run ID、JSONL 记录与 SQLite 元数据让智能体运行结束后仍可检查。</p>
  </article>
</section>

<section class="unio-flow">
  <h2>一个运行时，多个入口</h2>
  <ol>
    <li>从终端使用 `unio`、`unio exec` 或直接工具命令启动。</li>
    <li>CLI 把工作提交给 daemon，由 daemon 管理会话、运行、审批、工具、存储和 trace。</li>
    <li>智能体使用配置好的模型提供方；本地开发可使用 mock provider。</li>
    <li>安全策略决定工具调用是允许、拒绝还是等待审批。</li>
  </ol>
</section>

## 从这里开始

- [安装](install.md)：依赖、发布包与源码构建。
- [快速开始](quickstart.md)：本地第一组命令。
- [CLI](cli.md)：命令表面与示例。
- [工具与审批](tools-and-approvals.md)：直接工具与安全模式。
- [技能](skills.md)：技能发现与调用。
- [架构](architecture.md)：workspace 布局与运行流程。
