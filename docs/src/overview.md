# Unio

<section class="unio-hero">
  <div class="unio-kicker">Local intelligent agent runtime</div>
  <h1>Build, inspect, and automate from one terminal agent.</h1>
  <p>
    Unio is a Rust workspace for a general-purpose intelligent agent. It
    combines a CLI, local daemon, model providers, tool execution, approval
    policy, skills, storage, and trace events into one cohesive developer
    workflow.
  </p>
  <div class="unio-actions">
    <a href="install.html">Install</a>
    <a href="quickstart.html">Quickstart</a>
    <a href="architecture.html">Architecture</a>
  </div>
  <pre><code>cargo run -p unio -- exec "inspect README.md"
cargo run -p unio -- tool read --args path=README.md</code></pre>
</section>

<section class="unio-grid">
  <article>
    <h2>Agent Workflows</h2>
    <p>Run prompts, resume sessions, inspect traces, and keep conversation state
    attached to a workspace.</p>
  </article>
  <article>
    <h2>Tools With Policy</h2>
    <p>Read files, write files, and execute registered tools through an approval
    layer instead of unguarded shell access.</p>
  </article>
  <article>
    <h2>Skills</h2>
    <p>Load repository or user skills from `.unio/skills/` and invoke them
    through the same tool contract as other capabilities.</p>
  </article>
  <article>
    <h2>Observable Runs</h2>
    <p>Trace IDs, run IDs, JSONL records, and SQLite metadata make agent work
    inspectable after the command finishes.</p>
  </article>
</section>

<section class="unio-flow">
  <h2>One Runtime, Multiple Entry Points</h2>
  <ol>
    <li>Start from the terminal with `unio`, `unio exec`, or a direct tool
    command.</li>
    <li>The CLI submits work to the daemon, which owns sessions, runs,
    approvals, tools, storage, and trace events.</li>
    <li>The agent uses the configured model provider or the mock provider for
    local development.</li>
    <li>Security policy decides whether a tool call is allowed, denied, or
    waiting for approval.</li>
  </ol>
</section>

## Start Here

- [Install](install.md): requirements, releases, and source builds.
- [Quickstart](quickstart.md): first commands to run locally.
- [CLI](cli.md): command surface and examples.
- [Tools And Approvals](tools-and-approvals.md): direct tools and safety modes.
- [Skills](skills.md): skill discovery and invocation.
- [Architecture](architecture.md): workspace layout and runtime flow.
